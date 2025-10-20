function handleRequest(url, options) {
    fetch(url, options)
        .then(response => response.text())
        .then(html => {
            const parser = new DOMParser();
            const doc = parser.parseFromString(html, 'text/html');
            morphdom(document.head, doc.head);
            morphdom(document.body, doc.body);
            window.history.pushState({}, '', url);
        })
        .catch(error => console.error('Error fetching page:', error));
}

document.addEventListener('click', event => {
    const anchor = event.target.closest('a');
    if (anchor && anchor.href && anchor.target !== '_blank' && new URL(anchor.href).origin === window.location.origin) {
        event.preventDefault();
        handleRequest(anchor.href, { method: 'GET' });
    }
});

document.addEventListener('submit', event => {
    const form = event.target.closest('form');
    if (form) {
        event.preventDefault();
        const formData = new FormData(form);
        const method = form.method.toUpperCase();
        const url = form.getAttribute('action') || window.location.pathname;

        if (method === 'GET') {
            const params = new URLSearchParams(formData);
            handleRequest(`${url}?${params}`, { method: 'GET' });
        } else {
            handleRequest(url, {
                method: 'POST',
                body: formData
            });
        }
    }
});

console.log("Frontend script loaded and active.");
if (window.location.hostname === "127.0.0.1") {
    const socket = new WebSocket('ws://127.0.0.1:8080/devws');

    socket.onopen = function(e) {
        console.log("[open] Connection established");
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
        console.log(`[error]`);
    };
}