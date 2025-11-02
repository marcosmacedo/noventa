// Build WebSocket URL dynamically with current host and port
const protocol = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
const port = window.location.port ? ':' + window.location.port : '';
const socketUrl = protocol + window.location.hostname + port + '/devws';

let socket;

function connect() {
    socket = new WebSocket(socketUrl);

    socket.onopen = function(e) {
        console.log(`[open] Connection established to ${socketUrl}`);
    };

    socket.onmessage = function(event) {
        if (event.data === 'reload') {
            console.log("[devws.js] Received reload message. Triggering swup navigation.");
            if (window.swup) {
                window.swup.navigate(window.location.href, {
                    cache: false,
                    scroll: {
                        reset: false
                    }
                });
            } else {
                window.location.reload();
            }
        }
    };

    socket.onclose = function(event) {
        if (event.wasClean) {
            console.log(`[close] Connection closed cleanly, code=${event.code} reason=${event.reason}`);
        } else {
            console.log('[close] Connection died. Attempting to reconnect in 3 seconds...');
            setTimeout(connect, 3000);
        }
    };

    socket.onerror = function(error) {
        console.log(`[error] WebSocket error:`, error);
        socket.close();
    };
}

connect();