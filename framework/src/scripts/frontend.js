let currentPath = window.location.pathname + window.location.search;

function handleNavigation(newUrl) {
    const newPathURL = new URL(newUrl, window.location.origin);
    const newPath = newPathURL.pathname + newPathURL.search;

    if (newPath !== currentPath) {
        window.scrollTo(0, 0);
    }

    if (newPathURL.hash) {
        // Use requestAnimationFrame to ensure the DOM is updated before scrolling to the hash.
        requestAnimationFrame(() => {
            const element = document.querySelector(newPathURL.hash);
            if (element) {
                element.scrollIntoView();
            }
        });
    }
}

let showLoadingBarTimeout;
let loadingBarVisible = false;

function handleRequest(url, options, isPopState = false) {
    const fetchOptions = {
        ...options,
        headers: {
            ...options?.headers,
            'X-Requested-With': 'XMLHttpRequest'
        }
    };

    if (fetchOptions.headers['X-Dev-Reload']) {
        return;
    }

    const loadingBar = document.getElementById('xhr-loading-bar');

    if (loadingBar) {
        showLoadingBarTimeout = setTimeout(() => {
            loadingBarVisible = true;
            loadingBar.style.width = '0%';
            loadingBar.style.opacity = '1';
            // Start the loading animation
            setTimeout(() => {
                loadingBar.style.width = '70%';
            }, 100);
        }, 300);
    }

    fetch(url, fetchOptions)
        .then(response => {
            if (response.headers.has('X-Noventa-Redirect')) {
                const redirectHeader = response.headers.get('X-Noventa-Redirect');

                const redirectUrl = new URL(redirectHeader, window.location.origin);


                if (redirectUrl.origin === window.location.origin) {
                    // The server has told us to redirect. We will initiate a new request
                    // for the new URL. This new request will be responsible for updating
                    // the browser history when it completes successfully.

                    handleRequest(redirectUrl.href, { method: 'GET' }, false);
                } else {
                    // For external redirects, we do a full page load.
                    window.location.href = redirectUrl.href;
                }
                return null; // Stop processing the current response.
            }
            return response.text();
        })
        .then(html => {
            if (html === null) {
                return; // Stop processing if a redirect was handled
            }
            clearTimeout(showLoadingBarTimeout);
            if (loadingBar && loadingBarVisible) {
                loadingBar.style.width = '100%';
                setTimeout(() => {
                    loadingBar.style.opacity = '0';
                    loadingBarVisible = false;
                }, 200);
            }
            const parser = new DOMParser();
            const doc = parser.parseFromString(html, 'text/html');
            const morphdomOptions = {
                onBeforeNodeDiscarded: function(node) {
                    if (node.id === 'xhr-loading-bar' || node.id === 'xhr-loading-bar-styles') {
                        return false; // Don't discard the loading bar or its styles
                    }
                }
            };
            morphdom(document.head, doc.head, morphdomOptions);
            morphdom(document.body, doc.body, morphdomOptions);
            handleNavigation(url);
            if (!isPopState) {
                window.history.pushState({}, '', url);
                const newUrl = new URL(url, window.location.origin);
                currentPath = newUrl.pathname + newUrl.search;
            }
        })
        .catch(error => {
            clearTimeout(showLoadingBarTimeout);
            console.error('Error fetching page:', error);
            if (loadingBar && loadingBarVisible) {
                loadingBar.style.width = '100%';
                loadingBar.style.backgroundColor = 'red';
                loadingBar.style.opacity = '1';
                setTimeout(() => {
                    loadingBar.style.opacity = '0';
                    loadingBarVisible = false;
                    setTimeout(() => {
                        loadingBar.style.backgroundColor = ''; // Reset color
                    }, 200);
                }, 500);
            }
        });
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

document.addEventListener('DOMContentLoaded', () => {
    const loadingBar = document.createElement('div');
    loadingBar.id = 'xhr-loading-bar';
    document.body.appendChild(loadingBar);

    const styles = `
        #xhr-loading-bar {
            position: fixed;
            top: 0;
            left: 0;
            width: 0%;
            height: 2px;
            background-color: #3498db;
            transition: width 0.2s, opacity 0.2s;
            z-index: 9999;
            opacity: 0;
        }
    `;
    const styleSheet = document.createElement("style");
    styleSheet.id = 'xhr-loading-bar-styles';
    styleSheet.type = "text/css";
    styleSheet.innerText = styles;
    document.head.appendChild(styleSheet);
});