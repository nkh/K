// ─── Focus Management ───
(function() {
    'use strict';
// ─── Focus Management ───
const _focusState = {
    previousElement: null,
    releaseFn: null,
};

/**
 * Find all focusable elements inside a container.
 * @param {HTMLElement} container
 * @returns {HTMLElement[]}
 */
function _getFocusable(container) {
    const selector = 'button, input, select, textarea, [tabindex]:not([tabindex="-1"])';
    return Array.from(container.querySelectorAll(selector))
        .filter(el => {
            // Exclude hidden/disabled elements
            if (el.offsetParent === null && el.style.position !== 'fixed') return false;
            if (el.disabled) return false;
            return true;
        });
}

/**
 * Trap Tab/Shift+Tab focus within a container element.
 * Returns a cleanup function that removes the handler and restores focus.
 * @param {HTMLElement} container
 * @returns {Function} releaseFocus()
 */
function trapFocus(container) {
    // Save the currently focused element so we can restore it later
    _focusState.previousElement = document.activeElement;

    const handler = (e) => {
        if (e.key !== 'Tab') return;
        e.preventDefault();

        const focusable = _getFocusable(container);
        if (focusable.length === 0) return;

        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        const active = document.activeElement;
        const idx = focusable.indexOf(active);

        if (e.shiftKey) {
            // Shift+Tab: go backwards, wrap from first to last
            if (idx <= 0) {
                last.focus();
            } else {
                focusable[idx - 1].focus();
            }
        } else {
            // Tab: go forwards, wrap from last to first
            if (idx === -1 || idx >= focusable.length - 1) {
                first.focus();
            } else {
                focusable[idx + 1].focus();
            }
        }
    };

    document.addEventListener('keydown', handler, true);

    const releaseFn = () => {
        document.removeEventListener('keydown', handler, true);
        // Restore focus to previously focused element if it's still in the DOM
        if (_focusState.previousElement && _focusState.previousElement.isConnected) {
            _focusState.previousElement.focus();
        }
        _focusState.previousElement = null;
        _focusState.releaseFn = null;
    };

    _focusState.releaseFn = releaseFn;
    return releaseFn;
}

/**
 * Release the current focus trap if one is active.
 */
function releaseCurrentFocusTrap() {
    if (_focusState.releaseFn) {
        _focusState.releaseFn();
    }
}


    window.trapFocus = trapFocus;
    window.releaseCurrentFocusTrap = releaseCurrentFocusTrap;
})();
