// @ts-check

"use strict";

/**
 * This script starts the Elm application and communicates with it.
 * 
 * This script is needed to handle everything that cannot be done directly
 * within Elm, for example service worker registration and notification handling.
 */ 

const Elm = /** @type {any} */ (window).Elm;

/**
 * @param {number} ms 
 * @returns 
 */
function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * @param {string} message 
 */
function displayFatalError(message) {
    const appElement = document.getElementById("app");
    if (appElement === null) {
        console.error('Not found "app" element');
        return;
    }
    appElement.replaceChildren();

    appElement.textContent = message;
    appElement.style.padding = "20px";
    appElement.style.boxSizing = "border-box";
    appElement.style.width = "100%";
    appElement.style.textAlign = "center";
    appElement.style.fontSize = "20px";
}

if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/serviceWorker.js').then(async (registration) => {
        try {
            await main(registration);
        } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            displayFatalError(`An error occurred while starting the application. Error: ${message}`);
        }
    }).catch(error => {
        // Using console.error here so that debug.js can process this. (instead of letting the browser handle the error)
        console.error('Failed to register service worker:', error);
    });
} else {
    displayFatalError("This website requires service worker support, but it seems that your browser does not support service workers.");

    console.warn('Service workers are not supported in this browser.');
}

// app.ports.triggerNotification.subscribe(async function () {    
// });

/**
 * @param {ArrayBuffer} arrayBuffer contains the key data
 * @returns {string} A base64url encoded string
 */
function toPushApiCompatibleBase64(arrayBuffer) {
    return new Uint8Array(arrayBuffer)
        .toBase64({ alphabet: "base64url" })
        .replace(/=+$/, ''); // Remove any trailing '=' characters used for padding
}

/**
 * @param {PushSubscription} subscription 
 * @returns {string} A JSON string representing the subscription
 */
function subscriptionToJson(subscription) {
    const authKey = subscription.getKey('auth');
    const p256dhKey = subscription.getKey('p256dh');
    if (!authKey || !p256dhKey) {
        throw new Error('Push subscription is missing required keys (auth or p256dh).');
    }
    const auth = toPushApiCompatibleBase64(authKey);
    const p256dh = toPushApiCompatibleBase64(p256dhKey);

    return JSON.stringify({
        endpoint: subscription.endpoint,
        keys: {
            auth: auth,
            p256dh: p256dh
        }
    });
}

/**
 * 
 * @param {PushSubscription} subscription 
 */
async function sendSubscriptionToServer(subscription) {
    const subscriptionResponse = await fetch('/api/subscription', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: subscriptionToJson(subscription)
    });

    if (!subscriptionResponse.ok) {
        let errorMessage = `Failed to send push subscription to server.`;
        try {
            const errorText = await subscriptionResponse.json();
            errorMessage += ` Server sent: ${errorText}`;
        } catch (error) {
            console.info('Failed to parse error response from server as JSON. Error was:', error);
        }
        console.error(errorMessage);
        // TODO do something about this like save to the local storage whether the subscription
        // was sent to the server or not and retry later if it wasn't sent successfully
    }
}

/**
 * @param {ArrayBuffer | string} applicationServerKey Must be from the PushSubscriptionOptions type
 * @param {string} vapidPublicKey a base64 encoded string representing the VAPID public key
 */
function isMatchingApplicationServerKey(applicationServerKey, vapidPublicKey) {
    if (applicationServerKey instanceof ArrayBuffer) {
        const applicationServerKeyBase64 = toPushApiCompatibleBase64(applicationServerKey);

        console.log('Comparing applicationServerKey (base64):', applicationServerKeyBase64, 'with vapidPublicKey:', vapidPublicKey);
        return applicationServerKeyBase64 === vapidPublicKey;
    } else if (applicationServerKey == vapidPublicKey) {
        return true;
    } else {
        console.warn('Unexpected type of applicationServerKey in push subscription:', applicationServerKey);
        return false;
    }
}

/**
 * 
 * @param {ServiceWorkerRegistration} serviceWorkerRegistration 
 */
async function main(serviceWorkerRegistration) {
    const publicKeyResponse = await fetch('/api/public-key');
    if (!publicKeyResponse.ok) {
        throw new Error(`Failed to fetch VAPID public key from server. Status was ${publicKeyResponse.status}`);
    }
    const vapidPublicKey = await publicKeyResponse.text();
    console.log('Fetched VAPID public key from server:', vapidPublicKey);

    async function tryGetPushSubscription() {
        const subscription = await serviceWorkerRegistration.pushManager.getSubscription();
        console.log('serviceWorkerRegistration.pushManager.getSubscription:', subscription);
        if (subscription) {
            const applicationServerKey = subscription.options.applicationServerKey;
            console.log('applicationServerKey:', applicationServerKey);
            if (!applicationServerKey) {
                console.warn('Existing push subscription found, but it does not have an application server key. This is unexpected and may indicate a problem with the subscription. Resubscribing with the current VAPID public key.');
                await subscription.unsubscribe();
                return null;
            }
            if (!isMatchingApplicationServerKey(applicationServerKey, vapidPublicKey)) {
                console.warn('Existing push subscription found, but application server key does not match the current VAPID public key. Resubscribing with the new VAPID public key.');
                await subscription.unsubscribe();
                return null;
            } else {
                console.log('Existing push subscription found:', subscription);

                // Send subscription to the server just in case it was not sent before
                // or the server lost it during a restart
                try {
                    await sendSubscriptionToServer(subscription);
                } catch (error) {
                    console.error('Failed to send existing push subscription to server:', error);
                }
            }
        }
        return subscription;
    }

    const isSubscribed = Boolean(await tryGetPushSubscription());

    const app = Elm.Main.init({
        node: document.getElementById("app"),
        flags: {
            isSubscribed: isSubscribed
        }
    });

    async function getOrMakePushSubscription() {
        let subscription = await tryGetPushSubscription();
        if (!subscription) {
            subscription = await serviceWorkerRegistration.pushManager.subscribe({
                userVisibleOnly: true,
                applicationServerKey: vapidPublicKey
            });
            console.log('New push subscription created:', subscription);
        }

        return subscription;
    }

    app.ports.startWorker.subscribe(async function () {
        console.log('Requesting notification permission and registering service worker');
        let result = "failed";
        try {
            const permission = await Notification.requestPermission();
            
            if (permission !== 'granted') {
                console.warn('Notification permission was not granted. Permission is:', permission);
                return;
            }
            try {
                await getOrMakePushSubscription();
            } catch (error) {
                console.error('Service worker registration failed:', error);
                throw error;
            }
            result = "subscribed";
        } catch (error) {
            result = "failed";
            console.error('An error occurred while requesting notification permission or registering service worker:', error);
        } finally {
            app.ports.subscriptionResultHandler.send(result);
        }
    });

    
}

