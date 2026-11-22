# NAS-VM on the SDN via Cloud Hypervisor (100% rootless, no libvirt)

Boot a VM on the delonix SDN network (no firmware, no AppArmor, no
qemu:///system), with an IP discoverable via the SDN's DHCP and reachable by
any container on the same network.

## 1. Extract kernel+initrd from the cloud image (once; no download)
    export LIBGUESTFS_BACKEND=direct
    virt-get-kernel -a ~/.local/share/delonix/vm-images/delonix-vm-runtime-22.04-2026.06-amd64.qcow2 -o ./boot
    # → boot/vmlinuz-5.15.0-1101-kvm  (kernel `-kvm`, virtio built-in — ideal for CH)
    #   boot/initrd.img-5.15.0-1101-kvm

## 2. Boot on the SDN (direct kernel boot — no firmware needed)
    delonix vm create nasvm \
      --disk ~/.local/share/delonix/vm-images/delonix-vm-runtime-22.04-2026.06-amd64.qcow2 \
      --memory 1G --vcpus 2 \
      --backend cloud-hypervisor --network pnet \
      --kernel ./boot/vmlinuz-5.15.0-1101-kvm \
      --initrd ./boot/initrd.img-5.15.0-1101-kvm \
      --cmdline "console=ttyS0 root=/dev/vda1 rw" \
      --hostname nasvm --ssh-key @~/.ssh/id_ed25519.pub

    delonix describe vm nasvm    # Running, ip: 10.219.x.x (SDN's DHCP)

## 3. Reach the VM (from any container on the same network)
    delonix container run --net pnet -d --name probe alpine sleep 300
    delonix container exec probe ping -c2 <vm-ip>     # 0% loss
    delonix container exec probe nc -zv <vm-ip> 22     # open

## Notes
- Ubuntu cloud's `-kvm` kernel has virtio built in → `root=/dev/vda1` with no external modules.
- `--backend libvirt` would give a NON-SDN VM (libvirt's own network); the SDN is the native path.
- For the HOST to consume a service from the VM (e.g. SMB/NFS), publish it through the ingress (DNAT) — same as a
  container; L3 inside the SDN is already proven.

## 4. SMB storage e2e — VM serves samba, container consumes it (PROVEN)

VM provisioned with samba via cloud-init (`--user-data <file>`, the SDN's egress does the apt install):

    [global]
      map to guest = Bad User
      security = user
    [delonixnas]
      path = /srv/nas
      guest ok = yes
      read only = no
      force user = root

Since `mount -t cifs` runs in the HOST's netns, the VM's SMB port is published to the host through the
ingress and mounted via `127.0.0.1`:

    delonix net netns publish nasvm 4445:445 --ip <vm-ip>       # DNAT host:4445 → VM:445
    sudo -E delonix volume create nastest \                     # mount needs CAP_SYS_ADMIN
      --type cifs --server 127.0.0.1 --share delonixnas \
      --options "port=4445,guest,vers=3.0"
    sudo -E delonix container run -v nastest:/nas alpine \
      sh -c 'cat /nas/hello.txt; echo new > /nas/from-container.txt'
      # READS the file created inside the VM AND writes back — persisted on the VM's share.

Flow: `VM(samba, SDN) --smb--> ingress:4445 --cifs--> Storage vol --bind--> container`.
Teardown frees everything (unpublish + volume rm unmounts; `--disk`/data stay): container rm,
volume rm, netns unpublish, vm rm.
