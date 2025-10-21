let currentPath = window.location.pathname + window.location.search;

function handleRequest(url, options, isPopState = false) {
    fetch(url, options)
        .then(response => response.text())
        .then(html => {
            const parser = new DOMParser();
            const doc = parser.parseFromString(html, 'text/html');
            Idiomorph.morph(document.head, doc.head);
            Idiomorph.morph(document.body, doc.body);
            if (!isPopState) {
                window.history.pushState({}, '', url);
                currentPath = new URL(url).pathname + new URL(url).search;
            }
        })
        .catch(error => console.error('Error fetching page:', error));
}

document.addEventListener('click', event => {
    const anchor = event.target.closest('a');
    if (anchor && anchor.href && anchor.target !== '_blank') {
        const linkUrl = new URL(anchor.href);
        if (linkUrl.origin !== window.location.origin) {
            return;
        }

        // If the path and search params are the same, it's a hash link. Let the browser handle it.
        if (linkUrl.pathname === window.location.pathname && linkUrl.search === window.location.search) {
            return;
        }

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
    const newPath = window.location.pathname + window.location.search;
    // Only fetch if the path has changed, ignoring hash changes.
    if (newPath !== currentPath) {
        currentPath = newPath;
        handleRequest(window.location.href, { method: 'GET' }, true);
    }
});

console.log("Frontend script loaded and active.");