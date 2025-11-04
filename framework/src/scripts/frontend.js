document.addEventListener('DOMContentLoaded', () => {
    const initSwup = () => {
        try {
            const container = document.querySelector('#swup') ? 'swup' : 'body';
            const swup = new Swup({
                containers: [container],
                animateHistoryBrowsing: true,
                animationSelector: false,
                cache: true,
                native: true,
                requestHeaders: {
                    'X-Requested-With': 'swup',
                },
                plugins: [
                    new SwupPreloadPlugin(),
                    new SwupScriptsPlugin({
                        head: false,
                        body: true
                    }),
                    new SwupHeadPlugin({
                        awaitAssets: true,
                        persistAssets: true
                    }),
                ]
            });

            window.swup = swup;

            const handleRedirect = (response) => {
                if (response && response.headers.has('X-Noventa-Redirect')) {
                    const redirectHeader = response.headers.get('X-Noventa-Redirect');
                    const redirectUrl = new URL(redirectHeader, window.location.origin);
                    swup.navigate(redirectUrl.href, { cache: false });
                    return true;
                }
                return false;
            };

            swup.hooks.on('visit:start', (visit) => {
                if (swup.isPost) {
                    visit.scroll.reset = false;
                    swup.isPost = false;
                }
            });

            swup.hooks.on('page:load', (visit, { page }) => {
                handleRedirect(page.response);
            });

            swup.hooks.on('visit:end', (visit) => {
                swup.cache.delete(visit.from.url);
            });

            swup.hooks.on('link:anchor', (visit, { hash }) => {
                const target = document.querySelector(hash);
                if (target) {
                    target.scrollIntoView({
                        behavior: 'smooth'
                    });
                }
            });

            document.addEventListener('click', event => {
                const button = event.target.closest('button[type="submit"], input[type="submit"]');
                if (button) {
                    const form = button.form || button.closest('form');
                    if (form) {
                        event.preventDefault();
                        const formData = new FormData(form);
                        const method = form.method.toUpperCase();
                        const url = form.getAttribute('action') || window.location.pathname;

                        if (method === 'GET') {
                            const params = new URLSearchParams(formData);
                            swup.navigate(`${url}?${params.toString()}`);
                        } else {
                            swup.isPost = true;
                            fetch(url, {
                                method: 'POST',
                                body: formData,
                                headers: {
                                    'X-Requested-With': 'swup',
                                }
                            }).then(response => {
                                if (!handleRedirect(response)) {
                                    return response.text();
                                }
                            }).then(html => {
                                swup.cache.set(window.location.href, { 
                                    url: window.location.href, 
                                    html: html 
                                });

                                // Now navigate to it — swup will use the cached version
                                swup.navigate(window.location.href);
                            });
                        }
                    } else {
                        console.log('Warn: No form found for this submit button');
                    }
                }
            });

            const styles = `
                ::view-transition-old(*),
                ::view-transition-new(*) {
                    animation: none;
                }
            `;
            const styleSheet = document.createElement("style");
            styleSheet.type = "text/css";
            styleSheet.innerText = styles;
            document.head.appendChild(styleSheet);
        } catch (e) {
            if (e instanceof ReferenceError) {
                setTimeout(initSwup, 10);
            }
        }
    };
    initSwup();
});