## About

See the [blog post](https://github.com/PoorlyDefinedBehaviour/poorlydefinedbehaviour.github.io/blob/main/content/posts/deterministic_simulation_testing/index.md).

```console
running 1 test
[BUS] Simulator -> Replica(1) QUEUED StartProposal(V(1, 0))
[BUS] Simulator -> Replica(2) QUEUED StartProposal(V(2, 1))
[BUS] Simulator -> Replica(1) RECEIVED StartProposal(V(1, 0))
[BUS] Replica(1) -> Replica(1) QUEUED Prepare(RID(R1, P1))
[BUS] Replica(1) -> Replica(2) QUEUED Prepare(RID(R1, P1))
[BUS] Replica(1) -> Replica(3) QUEUED Prepare(RID(R1, P1))
[BUS] Replica(1) -> Replica(3) RECEIVED Prepare(RID(R1, P1))
[BUS] Replica(3) -> Replica(1) QUEUED PrepareResponse(RID(R1, P1), None, None)
[BUS] Simulator -> Replica(2) RECEIVED StartProposal(V(2, 1))
[BUS] Replica(2) -> Replica(1) QUEUED Prepare(RID(R2, P1))
[BUS] Replica(2) -> Replica(2) QUEUED Prepare(RID(R2, P1))
[BUS] Replica(2) -> Replica(3) QUEUED Prepare(RID(R2, P1))
[BUS] Replica(2) -> Replica(3) RECEIVED Prepare(RID(R2, P1))
[BUS] Replica(3) -> Replica(2) QUEUED PrepareResponse(RID(R2, P1), None, None)
[BUS] Replica(3) -> Replica(1) RECEIVED PrepareResponse(RID(R1, P1), None, None)
[BUS] Replica(3) -> Replica(2) RECEIVED PrepareResponse(RID(R2, P1), None, None)
[BUS] Replica(1) -> Replica(2) RECEIVED Prepare(RID(R1, P1))
[BUS] Replica(2) -> Replica(1) QUEUED PrepareResponse(RID(R1, P1), None, None)
[BUS] Replica(2) -> Replica(1) RECEIVED PrepareResponse(RID(R1, P1), None, None)
[BUS] Replica(1) -> Replica(1) QUEUED Accept(RID(R1, P1), 1, V(1, 0))
[BUS] Replica(1) -> Replica(2) QUEUED Accept(RID(R1, P1), 1, V(1, 0))
[BUS] Replica(1) -> Replica(3) QUEUED Accept(RID(R1, P1), 1, V(1, 0))
[BUS] Replica(2) -> Replica(1) RECEIVED Prepare(RID(R2, P1))
[BUS] Replica(1) -> Replica(2) QUEUED PrepareResponse(RID(R2, P1), None, None)
[BUS] Replica(1) -> Replica(3) RECEIVED Accept(RID(R1, P1), 1, V(1, 0))
[BUS] Replica(3) -> Replica(1) QUEUED AcceptResponse(RID(R1, P1), 1)
[BUS] Replica(1) -> Replica(1) RECEIVED Prepare(RID(R1, P1))
[BUS] Replica(1) -> Replica(1) QUEUED PrepareResponse(RID(R1, P1), None, None)
[BUS] Replica(1) -> Replica(1) RECEIVED PrepareResponse(RID(R1, P1), None, None)
[BUS] Replica(1) -> Replica(1) RECEIVED Accept(RID(R1, P1), 1, V(1, 0))
[BUS] Replica(1) -> Replica(1) QUEUED AcceptResponse(RID(R1, P1), 1)
[BUS] Replica(1) -> Replica(2) RECEIVED PrepareResponse(RID(R2, P1), None, None)
[BUS] Replica(2) -> Replica(1) QUEUED Accept(RID(R2, P1), 1, V(2, 1))
[BUS] Replica(2) -> Replica(2) QUEUED Accept(RID(R2, P1), 1, V(2, 1))
[BUS] Replica(2) -> Replica(3) QUEUED Accept(RID(R2, P1), 1, V(2, 1))
[BUS] Replica(2) -> Replica(3) RECEIVED Accept(RID(R2, P1), 1, V(2, 1))
[BUS] Replica(3) -> Replica(2) QUEUED AcceptResponse(RID(R2, P1), 1)
[BUS] Replica(1) -> Replica(2) RECEIVED Accept(RID(R1, P1), 1, V(1, 0))
[BUS] Replica(2) -> Replica(1) QUEUED AcceptResponse(RID(R1, P1), 1)
[BUS] Replica(2) -> Replica(2) RECEIVED Accept(RID(R2, P1), 1, V(2, 1))
[BUS] Replica(2) -> Replica(2) QUEUED AcceptResponse(RID(R2, P1), 1)
[BUS] Replica(2) -> Replica(1) RECEIVED Accept(RID(R2, P1), 1, V(2, 1))
[BUS] Replica(1) -> Replica(2) QUEUED AcceptResponse(RID(R2, P1), 1)
[BUS] Replica(3) -> Replica(2) RECEIVED AcceptResponse(RID(R2, P1), 1)
[BUS] Replica(1) -> Replica(1) RECEIVED AcceptResponse(RID(R1, P1), 1)
[BUS] Replica(2) -> Replica(2) RECEIVED Prepare(RID(R2, P1))
[BUS] Replica(2) -> Replica(2) QUEUED PrepareResponse(RID(R2, P1), Some(1), Some("V(2, 1)"))
[BUS] Replica(1) -> Replica(2) RECEIVED AcceptResponse(RID(R2, P1), 1)
[ORACLE] value accepted by majority of replicas: majority=2 RID(R2, P1) value=V(2, 1) replicas=[1, 3]
[BUS] Replica(2) -> Replica(2) RECEIVED PrepareResponse(RID(R2, P1), Some(1), Some("V(2, 1)"))
[BUS] Replica(2) -> Replica(2) RECEIVED AcceptResponse(RID(R2, P1), 1)
[ORACLE] value accepted by majority of replicas: majority=2 RID(R2, P1) value=V(2, 1) replicas=[1, 2, 3]
[BUS] Replica(3) -> Replica(1) RECEIVED AcceptResponse(RID(R1, P1), 1)
[ORACLE] value accepted by majority of replicas: majority=2 RID(R1, P1) value=V(1, 0) replicas=[3, 1]
```

## Reproducible bugs

Run tests with the command:

```
cargo t action_simulation -- --nocapture > out.txt
```

After a bug is found, include the seed in the test command to replay the same bug.

```
SEED=6261363621053372974 cargo t action_simulation -- --nocapture > out.txt
```

Modify `Replica::on_prepare` to accept proposal numbers that are not strictly greater than the min proposal number:

```rust
fn on_prepare(&mut self, input: PrepareInput) {
      - if input.proposal_number > self.min_proposal_number {
      -     ...
      - }
      + if input.proposal_number >= self.min_proposal_number {
      +     ...
      + }
    }
```

Modify `Replica::on_prepare_response` to not include the value returned by the replica with the greatest proposal number in the next accept request.

```rust
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

Modify `Replica::on_accept` to stop saving the state to durable storage.

```rust
fn on_accept(&mut self, input: AcceptInput) {
    if input.proposal_number >= self.state.min_proposal_number {
        let mut state = self.state.clone();
        state.accepted_proposal_number = Some(input.proposal_number);
        state.accepted_value = Some(input.value);
        self.storage.store(&state).unwrap();
        self.state = state;
      ...
    }
}
```

Modify `Replica::on_prepare_response` to mistakenly pick the first accepted value in the set of responses.

```rust
fn on_prepare_response(&mut self, input: PrepareOutput) -> u64 {
  ...
  -let value = ...
  +let value = req
  +              .responses
  +              .iter()
  +              .find(|response| response.accepted_proposal_number.is_some())
  +              .map(|response| response.accepted_value.clone().unwrap())
  +              .unwrap_or_else(|| req.proposed_value.clone().unwrap());
  ...
}
```

Modify `FileStorage::store` to stop flushing the file contents to disk after modifying the state.

```rust
fn store(&self, state: &contracts::DurableState) -> std::io::Result<()> {
    // Comment this line:
    // file.sync_all()?;
}
```

