# deploy/helm

Minimal Helm chart scaffold for Wave 3 cluster mode.

This chart is intentionally not a production promise yet. It captures the planned shapes from `WRIT_MASTER_BUILD_PLAN.md` §3.8/§20:

- shared `writ-gateway` Deployment and Service;
- sidecar-per-agent-pod example Deployment;
- ServiceAccount;
- policy ConfigMap mounted at `/etc/writ/writ.yaml`;
- Secret references for Postgres, object storage, SSO/SIEM/Vault/KMS integration points;
- default-deny-style NetworkPolicy scaffold.

Threat-model reminders:

- Writ ledgers are tamper-evident, not tamper-proof unless an external anchor/export is configured.
- MCP-proxy-only deployments can be bypassed by agents that do not use the proxy. Use process or cluster enforcement where bypass resistance matters.
- Prompt injection is out of scope; Writ limits blast radius and records decisions.

## Install sketch

```sh
helm install writ ./deploy/helm \
  --set ledger.postgres.existingSecret=writ-postgres \
  --set ledger.objectStore.existingSecret=writ-object-store
```

Required Secrets are deliberately external to this scaffold. Create them with your normal secret manager, External Secrets Operator, SealedSecrets, or air-gapped equivalent.
