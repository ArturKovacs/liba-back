
self.addEventListener('install', () => {
    console.log('Service Worker installed');
    self.skipWaiting();
});

self.addEventListener('push', (event) => {
    console.log('Push event received:', event);

    const message = event.data.text();

    self.registration.showNotification('Van Banán?', {
        body: String(message),
    });
})

self.addEventListener('activate', async () => {
    console.log('Service Worker activated!');

    // setInterval(async () => {
    //     console.log('Showing notification timeout');

    //     self.registration.showNotification('Hello Elm!', {
    //         body: 'Current time is: ' + new Date().toLocaleTimeString(),
    //     });

    // }, 10000);
});

self.addEventListener('notificationclick', (event) => {
    event.notification.close();

    // event.waitUntil(
    //     clients.openWindow('/')
    // );
});
