function handleRequest(url, options, isPopState = false) {
    fetch(url, options)
        .then(response => response.text())
        .then(html => {
            const parser = new DOMParser();
            const doc = parser.parseFromString(html, 'text/html');
            morphdom(document.head, doc.head);
            morphdom(document.body, doc.body);
            if (!isPopState) {
                window.history.pushState({}, '', url);
            }
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

window.addEventListener('popstate', event => {
    handleRequest(window.location.href, { method: 'GET' }, true);
});

console.log("Frontend script loaded and active.");