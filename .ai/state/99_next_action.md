# 99_next_action.md — program counter (OVERWRITE on every transition)
<!-- Only the main agent writes this, and never while a background task or subagent is running. -->

- Updated: YYYY-MM-DD HH:MM
- Commitment state: OPEN_REQUEST   <!-- OPEN_REQUEST | ACTIVE_PROMISE | RETRACTED_PREMISE | AWAITING_DECLARATION -->
- Current target: 00
- Last completed step: <witnessed: command + result>
- Next permitted action: <one sentence>
- Waiting on human: none   <!-- or: plan.ok | commit.ok | push.ok | land.ok | review.ok -->

## Checklist
- [x] :::done    Target 00 read-init-d
- [ ] :::active  Target 10 ticket
- [ ] :::gate    Target 20 plan approval (human)
- [ ] Target 30 red -> green
- [ ] Target 40 critic
- [ ] :::gate    Target 45 review approval (human)
- [ ] :::gate    Target 50 commit (human)
- [ ] :::gate    Target 60 push (human)
- [ ] Target 70 CI + bots quiescent
- [ ] :::gate    Target 80 land (human)
- [ ] Target 90 scar recorded

## Retraction log (append)
<!-- if a premise was retracted: which one, witness, what was revoked -->
