/** Full-page navigation. Its own module so tests can stand in for it (jsdom cannot navigate). */
export function navigateTo(url: string): void {
    window.location.assign(url);
}
