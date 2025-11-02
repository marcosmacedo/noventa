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

            swup.hooks.on('visit:start', (visit) => {
                if (swup.isPost) {
                    visit.scroll.reset = false;
                    swup.isPost = false;
                }
            });

            swup.hooks.on('page:load', (visit, { page,
                options }) => {
                if (page.response && page.response.headers.has('X-Noventa-Redirect')) {
                    const redirectHeader = page.response.headers.get('X-Noventa-Redirect');
                    const redirectUrl = new URL(redirectHeader, window.location.origin);
                    if (redirectUrl.origin === window.location.origin) {
                        swup.navigate(redirectUrl.href);
                    } else {
                        window.location.href = redirectUrl.href;
                    }
                }
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
                            swup.navigate(url, {
                                method: 'POST',
                                body: formData,
                                cache: false
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