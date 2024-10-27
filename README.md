## About

```console
---- simulation::tests::basic_simulation stdout ----
seed=7213850374640531345
replica 2 start proposal
replica 2 start proposal
replica 3 start proposal
[BUS] Simulator QUEUED StartProposal(V(2)) to replica 2
[BUS] replica 2 RECEIVED StartProposal(V(2)) from Simulator
[BUS] replica 2 QUEUED Prepare(RID(2, 1), 1) to replica 1
[BUS] replica 2 QUEUED Prepare(RID(2, 1), 1) to replica 2
[BUS] replica 2 QUEUED Prepare(RID(2, 1), 1) to replica 3
[BUS] Simulator QUEUED StartProposal(V(2)) to replica 2
[BUS] replica 1 RECEIVED Prepare(RID(2, 1), 1) from replica 2
[BUS] replica 1 QUEUED PrepareResponse(RID(2, 1), None, None) to replica 2
[BUS] Simulator QUEUED StartProposal(V(3)) to replica 3
[BUS] replica 2 RECEIVED Prepare(RID(2, 1), 1) from replica 2
[BUS] replica 2 QUEUED PrepareResponse(RID(2, 1), None, None) to replica 2
[BUS] replica 3 RECEIVED Prepare(RID(2, 1), 1) from replica 2
[BUS] replica 3 QUEUED PrepareResponse(RID(2, 1), None, None) to replica 2
[BUS] replica 2 RECEIVED StartProposal(V(2)) from Simulator
[BUS] replica 2 QUEUED Prepare(RID(2, 2), 2) to replica 1
[BUS] replica 2 QUEUED Prepare(RID(2, 2), 2) to replica 2
[BUS] replica 2 QUEUED Prepare(RID(2, 2), 2) to replica 3
[BUS] replica 2 RECEIVED PrepareResponse(RID(2, 1), None, None) from replica 1
[BUS] replica 3 RECEIVED StartProposal(V(3)) from Simulator
[BUS] replica 3 QUEUED Prepare(RID(3, 1), 1) to replica 1
[BUS] replica 3 QUEUED Prepare(RID(3, 1), 1) to replica 2
[BUS] replica 3 QUEUED Prepare(RID(3, 1), 1) to replica 3
[BUS] replica 2 RECEIVED PrepareResponse(RID(2, 1), None, None) from replica 2
[BUS] replica 2 QUEUED Accept(RID(2-1), 1, V(2)) to replica 1
[BUS] replica 2 QUEUED Accept(RID(2-1), 1, V(2)) to replica 2
[BUS] replica 2 QUEUED Accept(RID(2-1), 1, V(2)) to replica 3
[BUS] replica 2 RECEIVED PrepareResponse(RID(2, 1), None, None) from replica 3
[BUS] replica 1 RECEIVED Prepare(RID(2, 2), 2) from replica 2
[BUS] replica 1 QUEUED PrepareResponse(RID(2, 2), None, None) to replica 2
[BUS] replica 2 RECEIVED Prepare(RID(2, 2), 2) from replica 2
[BUS] replica 2 QUEUED PrepareResponse(RID(2, 2), None, None) to replica 2
[BUS] replica 3 RECEIVED Prepare(RID(2, 2), 2) from replica 2
[BUS] replica 3 QUEUED PrepareResponse(RID(2, 2), None, None) to replica 2
[BUS] replica 1 RECEIVED Prepare(RID(3, 1), 1) from replica 3
[BUS] replica 2 RECEIVED Prepare(RID(3, 1), 1) from replica 3
[BUS] replica 3 RECEIVED Prepare(RID(3, 1), 1) from replica 3
[BUS] replica 1 RECEIVED Accept(RID(2-1), 1, V(2)) from replica 2
[BUS] replica 2 RECEIVED Accept(RID(2-1), 1, V(2)) from replica 2
[BUS] replica 3 RECEIVED Accept(RID(2-1), 1, V(2)) from replica 2
[BUS] replica 2 RECEIVED PrepareResponse(RID(2, 2), None, None) from replica 1
[BUS] replica 2 RECEIVED PrepareResponse(RID(2, 2), None, None) from replica 2
[BUS] replica 2 QUEUED Accept(RID(2-2), 2, V(2)) to replica 1
[BUS] replica 2 QUEUED Accept(RID(2-2), 2, V(2)) to replica 2
[BUS] replica 2 QUEUED Accept(RID(2-2), 2, V(2)) to replica 3
[BUS] replica 2 RECEIVED PrepareResponse(RID(2, 2), None, None) from replica 3
[BUS] replica 1 RECEIVED Accept(RID(2-2), 2, V(2)) from replica 2
[BUS] replica 1 QUEUED AcceptResponse(RID(2-2), 2) to replica 2
[BUS] replica 2 RECEIVED Accept(RID(2-2), 2, V(2)) from replica 2
[BUS] replica 2 QUEUED AcceptResponse(RID(2-2), 2) to replica 2
[BUS] replica 3 RECEIVED Accept(RID(2-2), 2, V(2)) from replica 2
[BUS] replica 3 QUEUED AcceptResponse(RID(2-2), 2) to replica 2
[BUS] replica 2 RECEIVED AcceptResponse(RID(2-2), 2) from replica 1
[BUS] replica 2 RECEIVED AcceptResponse(RID(2-2), 2) from replica 2
[ORACLE] value accepted by majority of replicas: V(2)
[BUS] replica 2 RECEIVED AcceptResponse(RID(2-2), 2) from replica 3
[ORACLE] value accepted by majority of replicas: V(2)
```

## TODO

- Activity log like P-Lang has.
- Drop messages
- Delay messages
- Reorder messages
- Duplicate messages
