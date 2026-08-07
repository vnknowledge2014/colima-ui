# Colima Networking & DNS: Procedural Guide

## 1. Resolving Hostnames on Corporate VPNs
**Problem**: Containers cannot resolve internal corporate domains when connected to a VPN.
**Solution**: By default, Colima tries to use Google DNS (`8.8.8.8`) or Cloudflare (`1.1.1.1`).
**Procedure**:
1. Check the `dns` array in `~/.colima/<profile>/colima.yaml`.
2. To use the host's native DNS resolution (which includes VPN DNS):
   - Ensure `vmType: vz` is used.
   - Set `dns: []` (empty array).
3. Alternatively, explicitly set the corporate DNS server IP in the `dns` array.
4. Restart the instance.

## 2. Accessing the VM from other devices on the LAN
**Problem**: "I want to access my container from my phone on the same Wi-Fi network."
**Solution**: Colima's default networking only maps ports to `localhost` (127.0.0.1) on the Mac.
**Procedure**:
1. Ensure the user is using `vmType: vz`.
2. Enable `network_address: true` in the Colima config.
3. Restart the instance.
4. Use `colima status` to find the VM's assigned IP on the LAN (e.g., `192.168.x.x`).

## 3. Network Unreachable Errors inside VM
**Problem**: Containers have no outbound internet access.
**Procedure**:
1. Run `[RUN_APPROVE: colima ssh --profile default -c "ping -c 3 8.8.8.8"]`.
2. If it fails, the VM networking is wedged (common after Mac sleep/wake cycles with QEMU slirp).
3. The simplest fix is to restart the instance: `[EVENT_APPROVE: restart-instance | default]`.
