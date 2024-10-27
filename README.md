## About

```console
---- simulation::tests::basic_simulation stdout ----
seed=5723938277135031087
[BUS] Simulator QUEUED StartProposal(V(1)) to replica 1
[BUS] replica 1 RECEIVED StartProposal(V(1)) from Simulator
[BUS] replica 1 QUEUED Prepare(RID(1, 1), 1) to replica 1
[BUS] replica 1 QUEUED Prepare(RID(1, 1), 1) to replica 2
[BUS] replica 1 QUEUED Prepare(RID(1, 1), 1) to replica 3
[BUS] Simulator QUEUED StartProposal(V(1)) to replica 1
[BUS] replica 1 RECEIVED Prepare(RID(1, 1), 1) from replica 1
[BUS] replica 1 QUEUED PrepareResponse(RID(1, 1), None, None) to replica 1
[BUS] Simulator QUEUED StartProposal(V(3)) to replica 3
[BUS] replica 1 RECEIVED StartProposal(V(1)) from Simulator
[BUS] replica 1 QUEUED Prepare(RID(1, 2), 2) to replica 1
[BUS] replica 1 QUEUED Prepare(RID(1, 2), 2) to replica 2
[BUS] replica 1 QUEUED Prepare(RID(1, 2), 2) to replica 3
[BUS] replica 1 RECEIVED Prepare(RID(1, 2), 2) from replica 1
[BUS] replica 1 QUEUED PrepareResponse(RID(1, 2), None, None) to replica 1
[BUS] replica 1 RECEIVED PrepareResponse(RID(1, 2), None, None) from replica 1
[BUS] replica 3 RECEIVED Prepare(RID(1, 2), 2) from replica 1
[BUS] replica 3 QUEUED PrepareResponse(RID(1, 2), None, None) to replica 1
[BUS] replica 3 RECEIVED Prepare(RID(1, 1), 1) from replica 1
[BUS] replica 2 RECEIVED Prepare(RID(1, 2), 2) from replica 1
[BUS] replica 2 QUEUED PrepareResponse(RID(1, 2), None, None) to replica 1
[BUS] replica 1 RECEIVED PrepareResponse(RID(1, 1), None, None) from replica 1
[BUS] replica 2 RECEIVED Prepare(RID(1, 1), 1) from replica 1
[BUS] replica 1 RECEIVED PrepareResponse(RID(1, 2), None, None) from replica 2
[BUS] replica 1 QUEUED Accept(RID(1-2), 2, V(1)) to replica 1
[BUS] replica 1 QUEUED Accept(RID(1-2), 2, V(1)) to replica 2
[BUS] replica 1 QUEUED Accept(RID(1-2), 2, V(1)) to replica 3
[BUS] replica 3 RECEIVED Accept(RID(1-2), 2, V(1)) from replica 1
[BUS] replica 3 QUEUED AcceptResponse(RID(1-2), 2) to replica 1
[BUS] replica 1 RECEIVED Accept(RID(1-2), 2, V(1)) from replica 1
[BUS] replica 1 QUEUED AcceptResponse(RID(1-2), 2) to replica 1
[BUS] replica 1 RECEIVED AcceptResponse(RID(1-2), 2) from replica 3
[BUS] replica 1 RECEIVED PrepareResponse(RID(1, 2), None, None) from replica 3
[BUS] replica 1 RECEIVED AcceptResponse(RID(1-2), 2) from replica 1
[ORACLE] value accepted by majority of replicas: V(1)
[BUS] replica 2 RECEIVED Accept(RID(1-2), 2, V(1)) from replica 1
[BUS] replica 2 QUEUED AcceptResponse(RID(1-2), 2) to replica 1
[BUS] replica 3 RECEIVED StartProposal(V(3)) from Simulator
[BUS] replica 3 QUEUED Prepare(RID(3, 1), 1) to replica 1
[BUS] replica 3 QUEUED Prepare(RID(3, 1), 1) to replica 2
[BUS] replica 3 QUEUED Prepare(RID(3, 1), 1) to replica 3
[BUS] replica 1 RECEIVED AcceptResponse(RID(1-2), 2) from replica 2
[BUS] replica 3 RECEIVED Prepare(RID(3, 1), 1) from replica 3
[BUS] replica 1 RECEIVED Prepare(RID(3, 1), 1) from replica 3
[BUS] replica 2 RECEIVED Prepare(RID(3, 1), 1) from replica 3
```

## TODO

- Activity log like P-Lang has.
- Drop messages
- Delay messages
- Reorder messages
- Duplicate messages
