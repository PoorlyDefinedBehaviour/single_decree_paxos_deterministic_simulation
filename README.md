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

## Reproducible bugs

```
SEED=10557757033859378776 cargo t action_simulation -- --nocapture > out.txt
```

Modify `Replica::on_prepare` to accept proposal numbers that are not strictly greater than the min proposal number:
```diff
fn on_prepare(&mut self, input: PrepareInput) {
      - if input.proposal_number > self.min_proposal_number {
      -     ...
      - }
      + if input.proposal_number >= self.min_proposal_number {
      +     ...
      + }
    }
```

```
SEED=8105078063114987579 cargo t action_simulation -- --nocapture > out.txt
```

Modify `Replica::on_prepare_response` to not include the value returned by the replica with the greatest proposal number in the next accept request.

```diff
    fn on_prepare_response(&mut self, input: PrepareOutput) {
        ...
            -let value = req
            -    .responses
            -    .iter()
            -    .filter(|response| response.accepted_proposal_number.is_some())
            -    .max_by_key(|response| response.accepted_proposal_number)
            -    .map(|response| response.accepted_value.clone().unwrap())
            -    .unwrap_or_else(|| req.proposed_value.clone().unwrap());
            +let value = req.proposed_value.clone().unwrap();
          ...
    }
```

```
SEED=1582355565138672611 cargo t action_simulation -- --nocapture > out.txt
```

Modify `Replica::on_accept` to stop saving the state to durable storage.

```diff
fn on_accept(&mut self, input: AcceptInput) {
    if input.proposal_number >= self.state.min_proposal_number {
        self.state.accepted_proposal_number = Some(input.proposal_number);
        self.state.accepted_value = Some(input.value);
        -self.storage.store(&self.state);
        +// self.storage.store(&self.state);
      ...
    }
}
```

## TODO

- Activity log like P-Lang has.
- Drop messages
- Delay messages
- Reorder messages
- Duplicate messages
