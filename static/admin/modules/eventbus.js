// ─── Event Bus ───
// Central event emitter for cross-module communication.
window.VRW = window.VRW || {};
VRW.EventBus = {
    _listeners: {},
    on(event, fn) { (this._listeners[event] = this._listeners[event] || []).push(fn); },
    off(event, fn) { if (this._listeners[event]) this._listeners[event] = this._listeners[event].filter(f => f !== fn); },
    emit(event, ...args) { (this._listeners[event] || []).forEach(fn => fn(...args)); },
    once(event, fn) {
        const wrapper = (...args) => { fn(...args); this.off(event, wrapper); };
        this.on(event, wrapper);
    }
};
