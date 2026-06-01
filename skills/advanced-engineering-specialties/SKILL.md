---
name: advanced-engineering-specialties
slug: advanced-eng
version: 1.0.0
description: Specialized engineering skills for advanced scenarios. These are not mandatory for every change but apply when working in the relevant domain. Consult the relevant section when the task involves distributed systems, performance optimization, large-scale testing, production operations, or team processes.
---

# Advanced Engineering Specialties

This file complements `senior-engineer-standards/SKILL.md` (which is mandatory for all work) with skills that apply in specific domains. Consult the relevant section when the task falls into that category.

---

## 1. Performance and Scale

### 1.1. Resource Budgeting
Define CPU, memory, network, and latency constraints before optimizing. Measure before and after any performance change. A "performance improvement" that was not measured is not an improvement — it is a guess. Use profiling tools (not intuition) to identify bottlenecks.

### 1.2. Caching Strategy
Know when to cache, what to cache, and how to invalidate. Cache the right level — caching too close to the source wastes memory; caching too far away serves stale data. Prevent stale data, thundering herd (multiple cache-miss requests hitting the source simultaneously), and cache poisoning. Set TTLs explicitly. Cache misses should be rare and recoverable, not catastrophic.

### 1.3. Asynchronous Design
Prefer async patterns to prevent blocking, cascading failures, and resource exhaustion. Async is not the same as concurrent — async means not blocking the current thread while waiting; concurrent means multiple operations in progress simultaneously. Design for eventual consistency where appropriate. Every async boundary is a potential source of bugs — trace the error handling through every async chain.

### 1.4. Load Shedding and Backpressure
Design systems that degrade gracefully under stress. When overloaded, shed low-priority work (e.g., skip non-essential logging, defer batch processing) rather than failing catastrophically. Propagate backpressure upstream — if the downstream is slow, the upstream should slow down too, not flood it with requests. Define explicit degradation tiers: what breaks first, what breaks last.

### 1.5. Database Performance Engineering
Design indexes, query patterns, and connection pooling for production workloads. Use `EXPLAIN` or equivalent to understand query plans before optimizing. An index that helps reads but slows every write is not free — measure the trade-off. Avoid N+1 queries. Connection leaks are silent killers — always use connection pooling with explicit limits.

### 1.6. Distributed Systems Literacy
Understand CAP theorem: in a partition, you choose consistency or availability — you do not get both. Understand consensus protocols (Raft, Paxos) at the conceptual level — you do not need to implement them, but you need to know when you are relying on them. Know the consistency model of every data store you use (strong, eventual, causal). Design for partitions: what happens when two nodes cannot communicate? What happens when messages arrive out of order or are duplicated?

### 1.7. Capacity Planning
Model growth based on data, not guesses. Benchmark to find actual bottlenecks, then scale based on measured throughput. Distinguish horizontal scaling (add more nodes) from vertical scaling (bigger nodes). Know which your system supports. Set alerts before hitting limits, not after.

### 1.8. Latency Engineering
Optimize critical path latency. Measure percentiles (p50, p95, p99), not averages — averages hide tail latency. A system with p50=10ms but p99=5000ms is not fast; it is unpredictable. Identify and eliminate tail latency causes: GC pauses, lock contention, queue buildup, retry storms. Every hop adds latency — minimize network round-trips in critical paths.

---

## 2. Testing Methodologies

### 2.1. Property-Based Testing
Define properties and invariants that must always hold, then generate randomized inputs to stress-test them. Example: "for any valid terminal size (1..500 cols, 1..500 rows), resize should never panic." Catch edge cases you would never think to write manually. Use libraries like `proptest` (Rust) or `QuickCheck`. Property-based tests complement example-based tests — they do not replace them.

### 2.2. Contract Testing
Verify that service consumers and providers agree on API contracts independently, without requiring both services to be running simultaneously. Consumer tests verify "I send the right request format." Provider tests verify "I accept the expected request format and return the expected response format." This prevents integration failures from field name mismatches, type changes, or version drift.

### 2.3. Mutation Testing
Automatically introduce small bugs (mutations) into code and verify that the test suite catches them. If a mutation changes an `==` to `!=` and no test fails, the tests are not checking that comparison. This measures real test effectiveness, not just coverage percentages. High coverage with low mutation score means tests exist but do not assert meaningful behavior.

### 2.4. Behavior-Driven Development (BDD)
Express requirements as executable specifications in shared business language. Use Given/When/Then format to bridge the gap between domain experts and implementation. BDD tests serve as living documentation that stays synchronized with the code because they are the code. Prefer when requirements come from non-technical stakeholders who need to verify correctness.

### 2.5. Load and Stress Testing
Validate system behavior under expected and extreme load. Identify breaking points, resource exhaustion patterns, and degradation curves before production traffic hits. Load testing answers "does it work at normal volume?" Stress testing answers "where does it break?" Soak testing answers "does it leak memory over time?" Each answers a different question — use the right one.

### 2.6. Chaos Engineering
Intentionally inject failures (latency, errors, crashes, network partitions) in controlled environments. Validate that systems degrade gracefully and recover automatically without human intervention. Chaos testing proves resilience; without it, resilience is assumed, not verified. Start small (kill one process) before going big (simulate full network partition).

---

## 3. Operations and Process

### 3.1. Incident Response Preparedness
Define runbooks, escalation paths, and communication protocols before incidents happen. A runbook is a step-by-step guide: "if X is broken, check Y, then do Z." Runbooks should be written during calm times, not during emergencies. Practice with drills — a runbook that has never been tested is a hope, not a plan. Include: symptoms, diagnosis steps, mitigation steps, escalation triggers.

### 3.2. Stakeholder Communication
Translate technical trade-offs into business impact. When presenting options, frame them as: "Option A is faster to implement but will not scale past X users. Option B takes longer but handles 10x growth. Option C reuses existing infrastructure but limits customization." Provide options with risks, costs, and timelines — not just problems.

### 3.3. Mentorship and Knowledge Transfer
Document decisions and their rationale. Pair on complex work so knowledge is shared. Build team capability — a system understood by one person is a single point of failure. Write ADRs (Architecture Decision Records) for significant choices. Create onboarding documentation that gets new contributors productive quickly.

### 3.4. Project Estimation
Break work into verifiable chunks. Account for uncertainty by providing ranges, not single-point estimates. A task estimated at "2-4 hours" is honest. A task estimated at "3 hours" is almost always wrong. Update estimates as reality emerges — a stale estimate is worse than no estimate. Track estimation accuracy over time to calibrate.

### 3.5. Cross-Functional Collaboration
Work effectively with product, design, QA, and operations. Respect constraints from other domains — a design constraint is not an arbitrary limitation. Contribute to shared goals rather than optimizing locally for your own component. When a requirement conflicts with a technical constraint, surface the conflict early with trade-off analysis.

### 3.6. Ethical Engineering
Consider societal impact, bias, accessibility, and sustainability in technical decisions. A system that works well for most users but fails for users with disabilities is not working well. Consider data retention — storing everything "just in case" creates liability. Build systems that serve all users fairly, not just the majority.
