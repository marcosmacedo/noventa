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
            console.log("[devws.js] Received reload message. Fetching new content.");
            fetch(window.location.href, {
                headers: { 'X-Dev-Reload': 'true' }
            })
                .then(response => {
                    console.log("[devws.js] Fetched new content.");
                    return response.text();
                })
                .then(html => {
                    const parser = new DOMParser();
                    const doc = parser.parseFromString(html, 'text/html');
                    const newMainContainer = doc.getElementById('main_container');
                    if (newMainContainer) {
                        // Wrap existing body content to apply blur
                        const blurWrapper = document.createElement('div');
                        blurWrapper.id = 'devws-blur-wrapper';
                        while (document.body.firstChild) {
                            blurWrapper.appendChild(document.body.firstChild);
                        }
                        document.body.appendChild(blurWrapper);

                        // Apply a subtle blur to the wrapper
                        blurWrapper.style.filter = 'blur(3px)';
                        blurWrapper.style.transition = 'filter 0.3s ease-in-out';
                        
                        // Remove existing floating container if it exists
                        const existingFloatingContainer = document.getElementById('floating_main_container');
                        if (existingFloatingContainer) {
                            existingFloatingContainer.remove();
                        }

                        // Create a floating container for the new content
                        const floatingContainer = document.createElement('div');
                        floatingContainer.id = 'floating_main_container';
                        floatingContainer.innerHTML = newMainContainer.innerHTML;
                        floatingContainer.style.position = 'fixed';
                        floatingContainer.style.top = '50%';
                        floatingContainer.style.left = '50%';
                        floatingContainer.style.transform = 'translate(-50%, -50%)';
                        floatingContainer.style.zIndex = '1001';
                        floatingContainer.style.padding = '20px';
                        floatingContainer.style.borderRadius = '8px';
                        floatingContainer.style.boxShadow = '0 4px 30px rgba(0, 0, 0, 0.1)';
                        floatingContainer.style.backdropFilter = 'blur(5px)';
                        
                        document.body.appendChild(floatingContainer);

                    } else {
                        // Fallback to full page replacement if containers aren't found
                        Idiomorph.morph(document.documentElement, doc.documentElement, { restoreFocus: true });
                    }
                })
                .catch(error => console.error('Error fetching page for reload:', error));
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