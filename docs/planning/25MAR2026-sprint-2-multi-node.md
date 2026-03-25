# Sprint 2: Multi-Node Gossip & Seed Loading
**Date:** 25MAR2026
**Status:** APPROVED
**Author:** Claude Code

## Context

Sprint 1 delivered the core fuzzing pipeline and transport layer. However, nodes
can't actually discover each other or share data yet. This sprint closes that gap
by wiring the SWIM protocol loop, implementing the join procedure, loading seed
corpus files, and integrating gossip into the fuzz loop.

## Scope

### Step 1: Ping/PingAck Protocol Loop
- Add a `SwimController` that runs on a timer (gossip_interval)
- Each tick: select a random alive peer, send Ping, wait for PingAck
- If no PingAck within failure_timeout: mark peer Suspected
- If Suspected peer doesn't recover within suspicion_timeout: confirm Dead
- Implement PingReq (indirect ping) for robustness

### Step 2: Join Procedure
- On startup with `--seeds`: send Join message to each seed node
- Seed nodes respond with MembershipSync (their known peers)
- New node adds all received peers to its membership list
- New node announces itself to received peers

### Step 3: Seed Corpus Loading
- On `hivefuzz run`: load all files from the seeds directory into corpus
- Skip empty files and files over a size limit (1MB default)
- Log count of loaded seeds

### Step 4: Wire Gossip to Fuzz Loop
- Spawn gossip as a background tokio task alongside the fuzz loop
- Use channels to communicate between fuzz loop and gossip task
- Gossip task runs dissemination rounds on the configured interval
- Handle incoming messages (Ping → PingAck, Join → MembershipSync, etc.)

## Success Criteria

1. Two nodes on localhost discover each other via seed mechanism
2. SWIM failure detection marks a killed node as Dead
3. Seed corpus files are loaded and used as mutation sources
4. Gossip rounds run concurrently with the fuzz loop
5. All existing + new tests pass

## Risks

| Risk | Mitigation |
|------|-----------|
| Tokio task coordination complexity | Use simple mpsc channels, avoid shared state |
| Port conflicts in tests | Use port 0 everywhere |
| Timer drift under load | Tokio interval is good enough; don't over-engineer |
