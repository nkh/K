/// test/test_eventbus.js — Tests for EventBus module
require('./setup');

console.log('\n=== eventbus.js Tests ===\n');

const bus = VRW.EventBus;

// ── on/emit ──
console.log('on/emit tests');
let received = null;
const handler = (data) => { received = data; };
bus.on('test-event', handler);
bus.emit('test-event', { msg: 'hello' });
assertEq(received.msg, 'hello', 'listener receives data');

// Multiple listeners
let count = 0;
bus.on('counter', () => { count++; });
bus.on('counter', () => { count++; });
bus.emit('counter');
assertEq(count, 2, 'multiple listeners all fire');

// ── off ──
console.log('off tests');
received = null;
bus.off('test-event', handler);
bus.emit('test-event', { msg: 'world' });
assertEq(received, null, 'removed listener does not fire');

// ── once ──
console.log('once tests');
let onceCount = 0;
bus.once('once-test', () => { onceCount++; });
bus.emit('once-test');
assertEq(onceCount, 1, 'once fires on first emit');
bus.emit('once-test');
assertEq(onceCount, 1, 'once does not fire on second emit');

// ── emit with no listeners ──
console.log('emit with no listeners');
assert(() => { bus.emit('nonexistent-event'); }, 'emit with no listeners does not throw');

// ── Event isolation ──
console.log('event isolation');
let eventA = false, eventB = false;
bus.on('event-a', () => { eventA = true; });
bus.on('event-b', () => { eventB = true; });
bus.emit('event-a');
assert(eventA, 'event-a fired');
assert(!eventB, 'event-b did not fire');

// ── Cleanup ──
// Remove all listeners by re-emitting to clean state
bus.off('counter');
bus.off('once-test');
bus.off('event-a');
bus.off('event-b');

console.log('\n[eventbus.js] Tests complete');
