use std::{fs::File, io::{Cursor, Read}, sync::{Arc}};

use std::convert::Infallible;
use std::net::SocketAddr;
use futures::future::join_all;

use hyper::{Server, body::{Body, HttpBody}, service::{make_service_fn, service_fn}};
use hyper::server::conn;
use hyper::{Request, Response};
// use hyper_util::rt::TokioIo;
use tokio::{net::TcpListener, sync::Mutex};
use log::{error, info};

use web_push::*;

static PUBLIC_KEY: &str = "BJnk2lda5XbNnCGHOd_488hPYQUeqDo_kAsI3cKAphNAB1f7EUPuatikwUadbY10LrGioLpLTzrR2i5A1PTyUM4";

struct PushSender {
    subscriptions: Vec<SubscriptionInfo>,
    vapid_private_key: String,
    client: HyperWebPushClient,
}

impl PushSender {
    /// vapid_private_key should be a PEM-encoded EC private key
    fn new(vapid_private_key: String) -> Self {
        Self {
            subscriptions: Vec::with_capacity(20),
            vapid_private_key,
            client: HyperWebPushClient::new(),
        }
    }

    async fn send_push_message(&self, payload: &[u8], ttl: Option<u32>) -> Result<(), WebPushError> {

        let futures = self.subscriptions.iter().map(async |subscription_info| {
            let result = self.send_push_message_for_single(subscription_info, payload, ttl).await;
            if let Err(error) = result {
                error!("An error occured: {:?}", error);
            }
        });

        join_all(futures).await;

        Ok(())
    }

    async fn send_push_message_for_single(&self, subscription_info: &SubscriptionInfo, payload: &[u8], ttl: Option<u32>) -> Result<(), WebPushError> {
        let mut builder = WebPushMessageBuilder::new(subscription_info);

        builder.set_payload(ContentEncoding::Aes128Gcm, payload);
        
        if let Some(seconds) = ttl {
            builder.set_ttl(seconds);
        }

        let cursor = Cursor::new(&self.vapid_private_key);

        let mut sig_builder = VapidSignatureBuilder::from_pem(cursor, subscription_info).unwrap();

        sig_builder.add_claim("sub", "mailto:test@example.com");
        sig_builder.add_claim("foo", "bar");
        sig_builder.add_claim("omg", 123);

        let signature = sig_builder.build().unwrap();
        builder.set_vapid_signature(signature);

        self.client.send(builder.build()?).await
    }
}


async fn hello(mut req: Request<hyper::Body>, subscription_info: Arc<Mutex<Option<SubscriptionInfo>>>) -> Result<Response<Body>, Infallible> {
    match req.uri().path() {
        "/api/subscription" => {
            let data = req.body_mut().data().await;
            match data {
                Some(Ok(chunk)) => {
                    let mut subscription_info = subscription_info.lock().await;
                    *subscription_info = Some(serde_json::from_slice(&chunk).unwrap());
                    info!("Received subscription info: {:?}", subscription_info);
                },
                Some(Err(e)) => error!("Failed to read request body: {:?}", e),
                None => error!("Request body is empty"),
            }
        }
        "/api/message" => {
            // Handle message logic here
            info!("Received message");
        },
        _ => {
            info!("Received request for unknown path: {}", req.uri().path());
        }
    }

    Ok(Response::new("Hello, World!".into()))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {

    // For every connection, we must make a `Service` to handle all
    // incoming HTTP requests on said connection.
    let make_svc = make_service_fn(|_conn| {
        // This is the `Service` that will handle the connection.
        // `service_fn` is a helper to convert a function that
        // returns a Response into a `Service`.

        async {
            let subscription_info: Arc<Mutex<Option<SubscriptionInfo>>> = Arc::new(Mutex::new(None));
            Ok::<_, Infallible>(service_fn(move |req| {
                hello(req, subscription_info.clone())
            }))
        }
    });

    let addr = ([0, 0, 0, 0], 3001).into();

    let server = Server::bind(&addr).serve(make_svc);

    println!("Listening on http://{}", addr);

    server.await?;










    // -------------------------------------------------------

    // // We create a TcpListener and bind it to 127.0.0.1:3000
    // let listener = TcpListener::bind(addr).await?;



    // // We start a loop to continuously accept incoming connections
    // loop {
    //     let (stream, _) = listener.accept().await?;

    //     // Use an adapter to access something implementing `tokio::io` traits as if they implement
    //     // `hyper::rt` IO traits.
    //     let io = TokioIo::new(stream);

    //     // Spawn a tokio task to serve multiple connections concurrently
    //     tokio::task::spawn(async move {
    //         let mut subscription_info: Option<SubscriptionInfo> = None;

    //         // Finally, we bind the incoming connection to our `hello` service
    //         if let Err(err) = conn::Http::new()
    //             // `service_fn` converts our function in a `Service`
    //             .serve_connection(io, service_fn(move |req| {
    //                 hello(req)
    //             }))
    //             .await
    //         {
    //             eprintln!("Error serving connection: {:?}", err);
    //         }
    //     });
    // }


    // TODO: start a hyper server and 

    // let mut sender = PushSender::new(subscription_info, vapid_private_key);

    // if let Err(error) = result {
    //     error!("An error occured: {:?}", error);
    // }

    Ok(())
}
