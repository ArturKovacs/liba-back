use std::{fs::File, io::{Cursor, Read}};

use web_push::*;

static PUBLIC_KEY: &str = "BJnk2lda5XbNnCGHOd_488hPYQUeqDo_kAsI3cKAphNAB1f7EUPuatikwUadbY10LrGioLpLTzrR2i5A1PTyUM4";

struct PushSender {
    subscription_info: SubscriptionInfo,
    vapid_private_key: String,
    client: HyperWebPushClient,
}

impl PushSender {
    /// vapid_private_key should be a PEM-encoded EC private key
    fn new(subscription_info: SubscriptionInfo, vapid_private_key: String) -> Self {
        Self {
            subscription_info,
            vapid_private_key,
            client: HyperWebPushClient::new(),
        }
    }

    async fn send_push_message(&self, payload: &[u8], ttl: Option<u32>) -> Result<(), WebPushError> {
        let mut builder = WebPushMessageBuilder::new(&self.subscription_info);

        builder.set_payload(ContentEncoding::Aes128Gcm, payload);

        if let Some(seconds) = ttl {
            builder.set_ttl(seconds);
        }

        let cursor = Cursor::new(&self.vapid_private_key);

        let mut sig_builder = VapidSignatureBuilder::from_pem(cursor, &self.subscription_info).unwrap();

        sig_builder.add_claim("sub", "mailto:test@example.com");
        sig_builder.add_claim("foo", "bar");
        sig_builder.add_claim("omg", 123);

        let signature = sig_builder.build().unwrap();
        builder.set_vapid_signature(signature);

        self.client.send(builder.build()?).await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let mut subscription_info_file = String::new();
    let mut vapid_private_key: Option<String> = None;
    let mut push_payload: Option<String> = None;
    let mut encoding: Option<String> = None;
    let mut ttl: Option<u32> = None;
    
    let ece_scheme = match encoding.as_deref() {
        Some("aes128gcm") => ContentEncoding::Aes128Gcm,
        Some("aesgcm") => ContentEncoding::AesGcm,
        None => ContentEncoding::Aes128Gcm,
        Some(_) => panic!("Content encoding can only be 'aes128gcm' or 'aesgcm'"),
    };

    let ece_scheme = ContentEncoding::Aes128Gcm;
    
    // let mut file = File::open(subscription_info_file).unwrap();
    // let mut contents = String::new();
    // file.read_to_string(&mut contents).unwrap();
    // let subscription_info: SubscriptionInfo = serde_json::from_str(&contents).unwrap();

    let subscription_info = SubscriptionInfo {
        endpoint: "TODO get this from the client".to_string(),
        keys: SubscriptionKeys {
            p256dh: "TODO get this from the client".to_string(),
            auth: "TODO get this from the client".to_string(),
        },
    };

    let vapid_private_key = Some("TODO: read the private key from either a file or an environment variable".to_string());

    let ttl = Some(60);

    let mut builder = WebPushMessageBuilder::new(&subscription_info);

    if let Some(ref payload) = push_payload {
        builder.set_payload(ece_scheme, payload.as_bytes());
    } else {
        builder.set_payload(ece_scheme, "Hello world!".as_bytes());
    }

    if let Some(seconds) = ttl {
        builder.set_ttl(seconds);
    }

    if let Some(ref vapid_file) = vapid_private_key {
        let file = File::open(vapid_file).unwrap();

        let mut sig_builder = VapidSignatureBuilder::from_pem(file, &subscription_info).unwrap();

        sig_builder.add_claim("sub", "mailto:test@example.com");
        sig_builder.add_claim("foo", "bar");
        sig_builder.add_claim("omg", 123);

        let signature = sig_builder.build().unwrap();

        builder.set_vapid_signature(signature);
    };

    let client = HyperWebPushClient::new();

    let result = client.send(builder.build()?).await;

    if let Err(error) = result {
        println!("An error occured: {:?}", error);
    }

    Ok(())
}
