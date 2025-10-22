// Build WebSocket URL dynamically with current host and port
const protocol = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
const port = window.location.port ? ':' + window.location.port : '';
const socketUrl = protocol + window.location.hostname + port + '/devws';

// Connect to WebSocket
const socket = new WebSocket(socketUrl);

socket.onopen = function(e) {
    console.log(`[open] Connection established to ${socketUrl}`);
};

socket.onmessage = function(event) {
    if (event.data === 'reload') {
        console.log("[message] Reloading page content");
        fetch(window.location.href)
            .then(response => response.text())
            .then(html => {
                const parser = new DOMParser();
                const doc = parser.parseFromString(html, 'text/html');
                morphdom(document.head, doc.head);
                morphdom(document.body, doc.body);
            })
            .catch(error => console.error('Error fetching page for reload:', error));
    }
};

socket.onclose = function(event) {
    if (event.wasClean) {
        console.log(`[close] Connection closed cleanly, code=${event.code} reason=${event.reason}`);
    } else {
        console.log('[close] Connection died');
    }
};

socket.onerror = function(error) {
    console.log(`[error] WebSocket error:`, error);
};