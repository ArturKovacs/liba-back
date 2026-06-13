use futures::io;
use log::debug;
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, fs::TryLockError, io::Seek, path::Path, time::Duration};
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


const DB_FILE_PATH: &str = "./db.msgpack";

type Subscriptions = HashMap<SubscriptionId, SubscriptionRest>;

pub struct Database {
}

enum ModifyOperationResult {
    WritebackNeeded,
    OnlyRead
}

impl Database {
    pub fn new() -> Self {
        Self {
        }
    }

    async fn access_db(&self, operation: impl FnOnce(&mut Subscriptions) -> ModifyOperationResult) -> Result<Subscriptions, io::Error> {
        let mut open_options = std::fs::OpenOptions::new();
        open_options.read(true).write(true);
        let mut db_file = ExclusiveAccessFile::new(DB_FILE_PATH, open_options).await?;

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
