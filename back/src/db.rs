use futures::io;
use log::debug;
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, fs::TryLockError, io::Seek, path::{Path, PathBuf}, time::Duration};
use web_push::{SubscriptionInfo, SubscriptionKeys};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Floor(pub u32);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SubscriptionId {
    pub endpoint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionRest {
    pub keys: SubscriptionKeys,
    pub floors: HashSet<Floor>,
}

struct ExclusiveAccessFile {
    file: std::fs::File,
}


impl ExclusiveAccessFile {
    /// Given that the file is always opened through `LockedFile`, then this opens the
    /// target file with exclusive access (only one `LockedFile` instance will have it open at a time)
    /// 
    /// On Windows, this only works if the file is opened with one of `.read(true)`, `.read(true).append(true)`, or `.write(true)`.
    /// Files opened in append-only mode are not locked.
    /// 
    /// However, this is intended to be used on Linux only, where, according to the manual of `flock`,
    /// "A shared or exclusive lock can be placed on a file regardless of 
    /// the mode in which the file was opened."
    /// 
    ///  ### Parameters:
    /// - path: The path to the file that should be opened with exclusive access
    /// This finishes after successfully locking the file
    pub async fn new(path: impl AsRef<Path>, options: std::fs::OpenOptions) -> Result<Self, io::Error> {
        let path = path.as_ref();
        let file = match options.open(path) {
            Err(e) => {
                return Err(e);
            }
            Ok(f) => f
        };
        'lock_loop: loop {
            // The lock is release when the file is closed, so no need to explicitly unlock
            match file.try_lock() {
                Err(TryLockError::WouldBlock) => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    continue;
                }
                Err(e) => {
                    return Err(io::Error::other(e));
                }
                Ok(()) => { break 'lock_loop }
            }
        }
        Ok(Self {
            file: file
        })
    }

}


type Subscriptions = HashMap<SubscriptionId, SubscriptionRest>;

#[derive(Debug, Default)]
pub struct DatabaseOptions {
    pub db_file_path: Option<PathBuf>
}

pub struct Database {
    db_path: PathBuf
}

enum ModifyOperationResult {
    WritebackNeeded,
    OnlyRead
}

impl Database {
    pub fn new(options: DatabaseOptions) -> Self {
        const DB_FILE_PATH: &str = "./db.msgpack";

        Self {
            db_path: options.db_file_path.unwrap_or(DB_FILE_PATH.into())
        }
    }

    async fn access_db(&self, operation: impl FnOnce(&mut Subscriptions) -> ModifyOperationResult) -> Result<Subscriptions, io::Error> {
        let mut open_options = std::fs::OpenOptions::new();
        open_options.read(true).write(true).create(true);
        let mut db_file = ExclusiveAccessFile::new(&self.db_path, open_options).await?;

        let file_size = db_file.file.metadata().map(|m| m.len()).unwrap_or(0);
        if file_size == 0 {
            let mut serializer = rmp_serde::Serializer::new(&mut db_file.file);
            Subscriptions::new().serialize(&mut serializer).map_err(|e| io::Error::other(e))?;
            db_file.file.seek(std::io::SeekFrom::Start(0)).map_err(|e| io::Error::other(e))?;
        }

        // parse file
        let mut subscriptions: Subscriptions = rmp_serde::from_read(&mut db_file.file).map_err(|e| io::Error::other(e))?;

        let operation_result = (operation)(&mut subscriptions);
        let write_needed = matches!(operation_result, ModifyOperationResult::WritebackNeeded);

        if write_needed {
            db_file.file.seek(std::io::SeekFrom::Start(0)).map_err(|e| io::Error::other(format!("failed to `seek` while adding subscription, error: {}", e)))?;
            db_file.file.set_len(0).map_err(|e| io::Error::other(format!("failed to `set_len` while adding subscription, error: {}", e)))?;
    
            let mut serializer = rmp_serde::Serializer::new(db_file.file);
            subscriptions.serialize(&mut serializer).map_err(|e| io::Error::other(e))?;
        }

        Ok(subscriptions)
    }

    pub async fn add_subscription(&self, subscription_info: SubscriptionInfo, floor: Floor) -> Result<(), io::Error> {
        debug!(
            "Adding subscription. New subscription: {:?}",
            subscription_info
        );

        self.access_db(|subscriptions| {
            let subscription_id = SubscriptionId {
                endpoint: subscription_info.endpoint,
            };
            subscriptions
                .entry(subscription_id)
                .or_insert_with(|| SubscriptionRest {
                    keys: subscription_info.keys,
                    floors: HashSet::new(),
                })
                .floors
                .insert(floor);
            ModifyOperationResult::WritebackNeeded
        }).await.map(|_| ())
    }

    pub async fn remove_subscription(&self, subscription: &SubscriptionId, floor: Floor) -> Result<(), io::Error> {
        self.access_db(|subscriptions| {
            if let Some(subscription) = subscriptions.get_mut(subscription) {
                subscription.floors.remove(&floor);
            }
            ModifyOperationResult::WritebackNeeded
        }).await.map(|_| ())
    }

    /// Returns subscriptions that are subscribed to the given floor
    pub async fn get_subscriptions(&self, floor: Floor) -> Result<HashSet<SubscriptionInfo>, io::Error> {
        let subscriptions = self.access_db(|_| ModifyOperationResult::OnlyRead).await?;
        
        Ok(subscriptions
            .iter()
            .filter_map(|(subscription_id, subscription_rest)| {
                if subscription_rest.floors.contains(&floor) {
                    Some(SubscriptionInfo {
                        endpoint: subscription_id.endpoint.clone(),
                        keys: subscription_rest.keys.clone(),
                    })
                } else {
                    None
                }
            })
            .collect())
    }

    pub async fn get_floors_for_subscription(
        &self,
        subscription: &SubscriptionId,
    ) -> Result<HashSet<Floor>, io::Error> {
        let subscriptions = self.access_db(|_| ModifyOperationResult::OnlyRead).await?;

        Ok(subscriptions
            .get(subscription)
            .map(|subscription_rest| subscription_rest.floors.clone())
            .unwrap_or_default())
    }
}


mod tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use std::time::Instant;

    #[tokio::test]
    async fn test_exclusive_file_access_prevents_concurrent_access() {
        let test_file = "./test_exclusive_access.tmp";
        
        // Clean up before test
        let _ = std::fs::remove_file(test_file);
        
        // Create the test file
        std::fs::File::create(test_file).expect("Failed to create test file");

        let start = Instant::now();
        
        // Spawn two tasks trying to access the same file
        let task1 = {
            let test_file = test_file.to_string();
            tokio::spawn(async move {
                let mut options = std::fs::OpenOptions::new();
                options.read(true).write(true);
                let _lock = ExclusiveAccessFile::new(&test_file, options)
                    .await
                    .expect("Task 1 failed to acquire lock");
                
                // Hold the lock for 500ms
                tokio::time::sleep(Duration::from_millis(500)).await;
                Instant::now()
            })
        };

        let task2 = {
            let test_file = test_file.to_string();
            tokio::spawn(async move {
                // Give task1 time to acquire the lock first
                tokio::time::sleep(Duration::from_millis(50)).await;
                
                let mut options = std::fs::OpenOptions::new();
                options.read(true).write(true);
                let _lock = ExclusiveAccessFile::new(&test_file, options)
                    .await
                    .expect("Task 2 failed to acquire lock");
                
                Instant::now()
            })
        };

        let task1_released = task1.await.expect("Task 1 panicked");
        let task2_acquired = task2.await.expect("Task 2 panicked");

        let elapsed = start.elapsed();
        
        // Task 2 should have acquired the lock significantly after task 1 released it
        // This verifies that task 2 was blocked waiting for the lock
        assert!(
            task2_acquired >= task1_released,
            "Task 2 should have acquired lock after Task 1 released it"
        );
        
        // Total time should be roughly 500ms (task1) + retry time (task2), not concurrent
        assert!(
            elapsed.as_millis() >= 500,
            "Tasks should not run concurrently; elapsed: {:?}",
            elapsed
        );

        // Clean up
        let _ = std::fs::remove_file(test_file);
    }

    #[tokio::test]
    async fn test_exclusive_file_access_sequential_operations() {
        let test_file = "./test_sequential_access.tmp";
        
        // Clean up before test
        let _ = std::fs::remove_file(test_file);
        
        // Create the test file with initial content
        std::fs::write(test_file, "initial").expect("Failed to create test file");

        // First operation: read and verify initial content
        {
            let mut options = std::fs::OpenOptions::new();
            options.read(true).write(true);
            let _lock = ExclusiveAccessFile::new(test_file, options)
                .await
                .expect("First lock acquisition failed");
            // Lock is held and then released here
        }

        // Second operation: verify we can acquire the lock again
        {
            let mut options = std::fs::OpenOptions::new();
            options.read(true).write(true);
            let _lock = ExclusiveAccessFile::new(test_file, options)
                .await
                .expect("Second lock acquisition failed");
            // If we reach here, sequential access works correctly
        }

        // Clean up
        let _ = std::fs::remove_file(test_file);
    }

    #[tokio::test]
    async fn test_database_add_and_retrieve_subscription() {
        let db_file_path = Path::new("test_database_add_and_retrieve_subscription.msgpack");
        // Clean up database file before test
        let _ = std::fs::remove_file(db_file_path);
        
        let db_options = DatabaseOptions { db_file_path: Some(db_file_path.into()) };
        let db = Database::new(db_options);
        
        // Create a test subscription
        let subscription_info = SubscriptionInfo {
            endpoint: "https://example.com/push/subscription1".to_string(),
            keys: SubscriptionKeys {
                auth: "test_auth_key".to_string(),
                p256dh: "test_p256dh_key".to_string(),
            },
        };
        let floor = Floor(2);
        
        // Add subscription
        db.add_subscription(subscription_info.clone(), floor)
            .await
            .expect("Failed to add subscription");
        
        // Retrieve subscriptions for the floor
        let retrieved = db.get_subscriptions(floor)
            .await
            .expect("Failed to retrieve subscriptions");
        
        // Verify the subscription is there
        assert_eq!(retrieved.len(), 1, "Should have exactly one subscription");
        let retrieved_sub = retrieved.iter().next().expect("No subscription found");
        assert_eq!(retrieved_sub.endpoint, subscription_info.endpoint);
        assert_eq!(retrieved_sub.keys.auth, subscription_info.keys.auth);
        assert_eq!(retrieved_sub.keys.p256dh, subscription_info.keys.p256dh);
        
        // Clean up
        let _ = std::fs::remove_file(db_file_path);
    }

    #[tokio::test]
    async fn test_database_multiple_subscriptions_multiple_floors() {
        let db_file_path = Path::new("test_database_multiple_subscriptions_multiple_floors.msgpack");

        // Clean up database file before test
        let _ = std::fs::remove_file(db_file_path);
        
        let db_options = DatabaseOptions { db_file_path: Some(db_file_path.into()) };
        let db = Database::new(db_options);
        
        // Create test subscriptions
        let subscription_info = SubscriptionInfo {
            endpoint: "https://example.com/push/subscription1".to_string(),
            keys: SubscriptionKeys {
                auth: "test_auth_key".to_string(),
                p256dh: "test_p256dh_key".to_string(),
            },
        };
        
        let floor_1 = Floor(1);
        let floor_2 = Floor(2);
        let floor_3 = Floor(3);
        
        // Add the same subscription to multiple floors
        db.add_subscription(subscription_info.clone(), floor_1)
            .await
            .expect("Failed to add subscription to floor 1");
        
        db.add_subscription(subscription_info.clone(), floor_2)
            .await
            .expect("Failed to add subscription to floor 2");
        
        db.add_subscription(subscription_info.clone(), floor_3)
            .await
            .expect("Failed to add subscription to floor 3");
        
        // Verify subscriptions exist on all floors
        let retrieved_floor_1 = db.get_subscriptions(floor_1)
            .await
            .expect("Failed to retrieve subscriptions for floor 1");
        assert_eq!(retrieved_floor_1.len(), 1, "Floor 1 should have one subscription");
        
        let retrieved_floor_2 = db.get_subscriptions(floor_2)
            .await
            .expect("Failed to retrieve subscriptions for floor 2");
        assert_eq!(retrieved_floor_2.len(), 1, "Floor 2 should have one subscription");
        
        let retrieved_floor_3 = db.get_subscriptions(floor_3)
            .await
            .expect("Failed to retrieve subscriptions for floor 3");
        assert_eq!(retrieved_floor_3.len(), 1, "Floor 3 should have one subscription");
        
        // Verify we can get all floors for the subscription
        let subscription_id = SubscriptionId {
            endpoint: subscription_info.endpoint.clone(),
        };
        let retrieved_floors = db.get_floors_for_subscription(&subscription_id)
            .await
            .expect("Failed to retrieve floors for subscription");
        
        assert_eq!(retrieved_floors.len(), 3, "Subscription should be on 3 floors");
        assert!(retrieved_floors.contains(&floor_1));
        assert!(retrieved_floors.contains(&floor_2));
        assert!(retrieved_floors.contains(&floor_3));
        
        // Clean up
        let _ = std::fs::remove_file(db_file_path);
    }
}
