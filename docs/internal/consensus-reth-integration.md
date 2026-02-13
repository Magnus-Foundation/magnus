# Consensus-Reth Integration

This document describes the integration of reth with the Magnus
threshold-simplex consensus engine. The goal is to give a relatively low level
overview how data flows from the consensus layer to the execution layer and
back in a single node. For an overview of the threshold simplex engine
and its configuration refer to the `magnus_bft::threshold_simplex`
documentation.

## Data flow: block production and state transition

The simplex threshold consensus engine proposes, validates, and
executes new state transitions via 3 interfaces:

+ `Automaton`, to report the genesis digest, propose and validate blocks;
+ `Relay`, to broadcast proposed blocks to the network;
+ `Reporter`, to drive the state transition by hooking into the consensus
steps.

In Magnus, [`ExecutionDriver`] and its `Mailbox` provide all 3 of
these interfaces.

[`ExecutionDriver`]: ../crates/consensus/consensus-engine/src/consensus/application/actor.rs

### The execution driver

The [`ExecutionDriver`] allows the consensus engine to drive block production.
It wraps the handle to a reth execution node and calls the engine API methods
`new_payload` (to validate a block), and `fork_choice_updated` (to both trigger
the building of new payloads and to advance the chain).

### The syncer / marshaller

Block ingress from the consensus layer into the reth execution layer happens
via one of three methods:

1. on `Automaton::verify`,
2. on `Reporter::finalize`,
3. on reth execution layer p2p (which we will not go into here).

Both `Automaton::verify` and `Reporter::finalize` rely on a `syncer` actor
which is implemented through the marshal module. `Automaton::verify`
tries reading a proposed block from the syncer through the `subscribe` method.
`Reporter::finalize` on the other hand is performed by the syncer and forwards
a finalized block to the execution driver so that it can advance the state
machine.

### Genesis

The genesis block is read straight from the reth node and its digest
disseminated in the network.

```mermaid
sequenceDiagram
  participant CL as Magnus Consensus Engine
  participant ED as Execution Driver
  participant DB as Reth Database Provider

  CL->>ED: Automaton::genesis
  ED->>DB: BlockReader::block_by_number(0)
  DB->>ED: genesis
  ED->>CL: genesis.digest
```

### Propose

Proposing new blocks is done directly via reth's payload builder, not through
the consensus engine. The execution driver derives the payload ID from the
parent digest. This way, if payload building takes too long and the consensus
engine cancels the proposal task, once consensus cycles back to the original
node it can pick up the payload that was kicked off before. This is assuming
that all other consensus nodes would equally struggle building a block in time.

`last_finalized` mentioned here refers to `last_finalized` set during the
*Finalize* step.

```mermaid
sequenceDiagram
  participant CL as Magnus Consensus Engine
  participant ED as Execution Driver
  participant SY as Magnus Syncer
  participant EB as Reth Payload Builder
  participant EE as Reth Consensus Engine

  CL->>ED: Automaton::propose(view, parent.digest)
  ED->>EE: fork_choice_updated(head = parent, finalized = last_finalized)
  ED->>SY: Subscribe(parent.digest)
  SY->>ED: parent
  ED->>EB: send_new_payload(id(parent), parent_hash)
  ED->>EB: proposal := resolve_id(payload_id)
  ED->>CL: proposal.digest
  ED->>EE: fork_choice_updated(head = proposal, finalized = last_finalized)
  CL->>ED: Relay::broadcast(proposal.digest)
  ED->>SY: broadcast(proposal)
```

### Verify

`last_finalized` mentioned here refers to `last_finalized` set during the
*Finalize* step.

```mermaid
sequenceDiagram
  participant CL as Magnus Consensus Engine
  participant ED as Execution Driver
  participant SY as Magnus Syncer
  participant EE as Reth Consensus Engine

  CL->>ED: Automaton::verify(parent, payload.digest)
  ED->>EE: fork_choice_updated(head = parent, finalized = last_finalized)
  ED->>SY: Subscribe(parent.digest)
  SY->>ED: parent
  ED->>SY: Subscribe(payload.digest)
  SY->>ED: payload
  ED->>ED: static_checks(parent, payload)
  ED->>EE: new_payload(payload)
  EE->>ED: payload_status
  ED->>CL: is_valid(static_checks, payload_status)
  ED->>EE: if valid: fork_choice_updated(head = proposal, finalized = last_finalized)
  ED->>SY: if valid: verified(block) // cache block
```

### Finalize

```mermaid
sequenceDiagram
  participant CL as Magnus Consensus Engine
  participant ED as Execution Driver
  participant SY as Magnus Syncer
  participant EE as Reth Consensus Engine

  CL->>ED: Reporter::finalize(block)
  ED->>EE: new_payload(block)
  EE->>ED: payload_status
  ED->>ED: last_finalized := block.digest
```

## Implementation details

### State Storage

The Magnus node implementation does not handle any state outside of
the default state storage provided by the consensus layer and reth.

### Consensus blocks and execution blocks

The atomic unit of communication in Magnus networks are blocks and
their digests. Blocks must implement the interface `EncodeSize`, `Read`,
`Write`, `Block`, `Committable`, and `Digestible`. Digests require
`Digest` (and a number of traits that follow from there).

To keep things simple and to keep a 1-to-1 relationship of block digests at
the consensus level and block hashes at the execution level, the
Magnus [consensus `Block`] is a refinement type of a reth
`SealedBlock`, while the [consensus `Digest`] is a refinement type of an
alloy-primitives `B256`.

["consensus" `Block`]: ../crates/consensus/consensus-engine/src/consensus/block.rs
["consensus" `Digest`]: ../crates/consensus/consensus-engine/src/consensus/digest.rs

### Async runtimes

Both the consensus layer and reth expect to initialize a tokio runtime and execute on top
of it. Luckily, reth does not bind as tightly to the runtime and so reth
commands can be launched from within the context of a Magnus
`Runner::start` executed closure. The `reth_glue` module effectively
re-implements the various reth `CliRunner::run_*` methods.

[`reth_glue`]: ../crates/consensus/consensus-engine/src/
