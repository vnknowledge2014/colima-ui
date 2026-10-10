# Kubernetes (K3s) in Colima: Quirks & Procedures

When running K3s inside Colima, there are specific quirks and workflows you must follow compared to a standard K8s cluster.

## 1. Ingress and Traefik
K3s comes with Traefik pre-installed as the default Ingress controller.
**Problem**: Users often try to install Nginx Ingress and it conflicts on port 80/443.
**Solution**: 
- If using Traefik (recommended): Just create standard `Ingress` resources. No extra setup needed.
- If the user *must* use Nginx Ingress: You must start the Colima instance with Traefik disabled.
  `colima start --kubernetes --kubernetes-disable traefik`
  *(Note: This requires CLI intervention as the UI checkbox currently enables default K3s).*

## 2. Port Forwarding and Host Access
**Problem**: "I deployed a NodePort service but I can't reach it on `localhost`."
**Solution**:
- By default, Colima maps `localhost` to the VM. 
- However, for Kubernetes services, it's highly recommended to use `kubectl port-forward` instead of relying on NodePort or LoadBalancer IPs, especially in VZ mode.
- **Procedure**: `kubectl port-forward svc/my-service 8080:80`

## 3. Multiple Colima Profiles with Kubernetes
**Problem**: Running two Colima instances (e.g., `default` and `dev`) both with `--kubernetes` will cause port conflicts on the host (port 6443 for API server).
**Solution**: 
- Only run one Kubernetes-enabled Colima instance at a time.
- Stop the other instance: `colima stop <other-profile>`.

## 4. Node Labels and Selectors
Colima nodes are labeled with `colima`.
**Procedure**: If you need to force a deployment to run specifically on the Colima node (useful in hybrid setups), use:
```yaml
nodeSelector:
  kubernetes.io/hostname: colima
```

## 5. Storage Classes
K3s in Colima uses `local-path` as the default StorageClass.
**Procedure**: When creating PersistentVolumeClaims (PVCs), you don't need to specify a storage class name; `local-path` is used automatically and provisions storage directly on the Colima VM's disk.
