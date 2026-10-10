# Kubernetes RBAC & Security: Procedural Guide

## 1. Minimal Service Accounts
**Problem**: By default, pods run using the `default` ServiceAccount, which might have overly broad permissions or lack the specific permissions needed to talk to the K8s API.
**Procedure**:
1. Create a dedicated ServiceAccount:
   `kubectl create serviceaccount my-app-sa`
2. Assign it to your deployment:
   ```yaml
   apiVersion: apps/v1
   kind: Deployment
   spec:
     template:
       spec:
         serviceAccountName: my-app-sa
   ```

## 2. Binding Roles (RBAC)
**Problem**: The custom ServiceAccount needs permission to read ConfigMaps or Secrets.
**Procedure**:
1. Create a Role:
   ```yaml
   apiVersion: rbac.authorization.k8s.io/v1
   kind: Role
   metadata:
     namespace: default
     name: config-reader
   rules:
   - apiGroups: [""]
     resources: ["configmaps"]
     verbs: ["get", "watch", "list"]
   ```
2. Bind the Role to the ServiceAccount:
   `kubectl create rolebinding my-app-binding --role=config-reader --serviceaccount=default:my-app-sa`

## 3. Security Contexts
**Problem**: Containers shouldn't run as root for security reasons.
**Procedure**: Enforce non-root execution in the Pod spec:
```yaml
spec:
  securityContext:
    runAsNonRoot: true
    runAsUser: 1000
  containers:
  - name: myapp
    image: myapp:latest
    securityContext:
      allowPrivilegeEscalation: false
      readOnlyRootFilesystem: true
```
*Note: If `readOnlyRootFilesystem` is true, you must mount `emptyDir` volumes for any paths the app needs to write to (like `/tmp` or `/var/run`).*
