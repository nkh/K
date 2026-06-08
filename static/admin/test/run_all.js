/// test/run_all.js — Test runner for vrw web UI JavaScript tests.
/// Usage: node test/run_all.js
'use strict';

const fs = require('fs');
const path = require('path');

console.log('╔══════════════════════════════════════════════╗');
console.log('║     vrw Web UI JavaScript Test Suite        ║');
console.log('╚══════════════════════════════════════════════╝\n');

// Load setup (mocks + loads all modules)
require('./setup');

// Discover test files
const testDir = __dirname;
const files = fs.readdirSync(testDir)
    .filter(f => f.startsWith('test_') && f.endsWith('.js') && f !== 'run_all.js')
    .sort();

console.log('Found ' + files.length + ' test files:\n');

let totalPassed = 0;
let totalFailed = 0;
let fileErrors = 0;

const startTime = Date.now();

for (const file of files) {
    const filePath = path.join(testDir, file);
    const beforePassed = globalThis._testPassed;
    const beforeFailed = globalThis._testFailed;

    try {
        require(filePath);
    } catch (e) {
        console.error('  ERROR loading ' + file + ': ' + e.message);
        fileErrors++;
        globalThis._testFailed++;
    }

    const passed = globalThis._testPassed - beforePassed;
    const failed = globalThis._testFailed - beforeFailed;
    totalPassed += passed;
    totalFailed += failed;

    const status = failed === 0 && fileErrors === 0 ? '  PASS' : '  FAIL';
    console.log('  [' + file.replace('test_', '').replace('.js', '') + '] ' + passed + ' passed, ' + failed + ' failed');
}

const elapsed = Date.now() - startTime;

console.log('\n╔══════════════════════════════════════════════╗');
console.log('║  Results: ' + String(totalPassed).padStart(4) + ' passed, ' + String(totalFailed).padStart(4) + ' failed       ║');
console.log('║  Files:   ' + String(files.length).padStart(4) + ' total,  ' + String(fileErrors).padStart(4) + ' errors       ║');
console.log('║  Time:    ' + (elapsed + 'ms').padStart(10) + '                  ║');
console.log('╚══════════════════════════════════════════════╝\n');

if (totalFailed > 0 || fileErrors > 0) {
    console.error('SOME TESTS FAILED');
    process.exit(1);
} else {
    console.log('ALL TESTS PASSED');
}
