# Install and point kubectl at Colima

## Symptom

The Kubernetes pages are empty, or you see `kubectl: command not found` or
*The connection to the server localhost:8080 was refused*.

## Cause

Two separate things are needed and either can be missing:

1. The `kubectl` binary on your host.
2. A Kubernetes cluster inside the Colima VM. Colima does **not** start one by
   default — it has to be enabled per profile.

The `localhost:8080` message specifically means kubectl ran fine but has no
context configured, so it fell back to its built-in default address.

## Fix

Install the client:

```bash
brew install kubectl
kubectl version --client
```

Then enable Kubernetes in the VM and select its context:

```bash
colima start --kubernetes
kubectl config get-contexts
kubectl config use-context colima
kubectl get nodes
```

You can also turn Kubernetes on from Settings → Colima Config. That writes the
setting to `colima.yaml`; it takes effect on the next restart of the instance.

Enabling Kubernetes adds roughly 1 GiB of memory use and a minute to startup, so
leave it off on profiles that only run containers.

## Related

- [Start a Colima instance](start-colima)
- [Performance tuning](performance-tuning)
