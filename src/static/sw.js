self.addEventListener('push', event => {
  // Parse incoming data payload from backend
  const data = event.data ? event.data.json() : { title: 'Default Title', body: 'New Update!' };

  // Display the native OS notification
  event.waitUntil(
    self.registration.showNotification(data.title, {
      body: data.body,
      icon: '/icon.png', // Optional path to an icon image
      badge: '/badge.png' // Optional path to a monochrome status bar icon
    })
  );
});

// Optional: Handle what happens when a user clicks the notification
self.addEventListener('notificationclick', event => {
  event.notification.close(); // Close the notification banner
  event.waitUntil(
    clients.openWindow('https://yourwebsite.com') // Redirect user
  );
});
