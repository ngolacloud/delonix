//! `NodeService` — what answers on the socket, gRPC and HTTP/JSON alike.

use std::pin::Pin;
use std::sync::Arc;

use axum::response::IntoResponse;
use tonic::{Request, Response, Status};

use crate::proto::v1::container_service_server::{ContainerService, ContainerServiceServer};
use crate::proto::v1::image_service_server::{ImageService, ImageServiceServer};
use crate::proto::v1::network_service_server::{NetworkService, NetworkServiceServer};
use crate::proto::v1::node_service_server::{NodeService, NodeServiceServer};
use crate::proto::v1::operation_service_server::{OperationService, OperationServiceServer};
use crate::proto::v1::pod_service_server::{PodService, PodServiceServer};
use crate::proto::v1::stack_service_server::{StackService, StackServiceServer};
use crate::proto::v1::storage_service_server::{StorageService, StorageServiceServer};
use crate::proto::v1::virtual_machine_service_server::{
    VirtualMachineService, VirtualMachineServiceServer,
};
use crate::proto::v1::volume_service_server::{VolumeService, VolumeServiceServer};
use crate::proto::v1::{
    ApiRoot, Capacity, Event, GetApiRootRequest, GetCapacityRequest, GetHealthRequest,
    GetNodeInfoRequest, Health, ListProvidersRequest, ListProvidersResponse, NodeInfo,
    WatchEventsRequest,
};
use crate::proto::v1::{ApplyStackRequest, DestroyStackRequest, PlanStackRequest, StackPlan};
use crate::proto::v1::{
    BuildImageRequest, DeleteImageRequest, GetImageRequest, Image, ListImagesRequest,
    ListImagesResponse, PullImageRequest, PushImageRequest, ScanImageRequest,
};
use crate::proto::v1::{
    CancelOperationRequest, GetOperationRequest, ListOperationsRequest, ListOperationsResponse,
    WatchOperationRequest,
};
use crate::proto::v1::{
    CephStatus, CreateObjectStoreUserRequest, GetCephStatusRequest, GetObjectStoreBucketRequest,
    ListCephfsSubvolumesRequest, ListCephfsSubvolumesResponse, ListObjectStoreBucketsRequest,
    ListObjectStoreBucketsResponse, ListRbdImagesRequest, ListRbdImagesResponse, ObjectStoreBucket,
    ObjectStoreUser,
};
use crate::proto::v1::{
    ConnectContainerRequest, Container, CreateNetworkRequest, DeleteNetworkRequest,
    DisconnectContainerRequest, GetNetworkRequest, ListNetworksRequest, ListNetworksResponse,
    Network, Operation,
};
use crate::proto::v1::{
    ConsoleRequest, ConsoleResponse, CreateSnapshotRequest, CreateVirtualMachineRequest,
    DeleteSnapshotRequest, DeleteVirtualMachineRequest, GetVirtualMachineRequest,
    ListSnapshotsRequest, ListSnapshotsResponse, ListVirtualMachinesRequest,
    ListVirtualMachinesResponse, PauseVirtualMachineRequest, RestoreSnapshotRequest,
    ResumeVirtualMachineRequest, StartVirtualMachineRequest, StopVirtualMachineRequest,
    VirtualMachine,
};
use crate::proto::v1::{
    ContainerStats, CreateContainerRequest, DeleteContainerRequest, ExecRequest, ExecResponse,
    GetContainerRequest, KillContainerRequest, ListContainersRequest, ListContainersResponse,
    LogChunk, LogsRequest, StartContainerRequest, StatsRequest, StopContainerRequest,
    UpdateContainerRequest, WaitContainerRequest, WaitContainerResponse,
};
use crate::proto::v1::{
    CreatePodRequest, DeletePodRequest, GetPodRequest, ListPodsRequest, ListPodsResponse, Pod,
};
use crate::proto::v1::{
    CreateVolumeRequest, DeleteVolumeRequest, GetVolumeRequest, ListVolumesRequest,
    ListVolumesResponse, Volume,
};
use crate::node;

/// The service. Its answers are computed from the engine's declarations and this
/// host's probes at call time; the one piece of injected state is the VM port
/// (ADR-0040 A5), through which the VM reads reach the composed engine instead of
/// the `delonix-vm` adapter. Cloned once per gRPC sub-service in [`router_with`]
/// (an `Arc`, so every clone shares the one port).
#[derive(Clone)]
pub struct NodeApi {
    vms: Arc<dyn crate::vms::VmPort>,
    store: Arc<dyn crate::store::StorePort>,
    run_resolver: Arc<dyn crate::run_resolver::RunResolver>,
    net: Arc<dyn crate::net_ops::NetworkOps>,
    images: Arc<dyn crate::image_repo::ImageRepo>,
    providers: Arc<dyn crate::provider_reports::ProviderReports>,
    linux: Arc<dyn crate::linux_lifecycle::LinuxLifecycle>,
}

/// Runs a host probe off the runtime's workers: every `NodeService` answer
/// reads files, cgroups and provider probes, which block.
async fn blocking<T: Send + 'static>(
    what: &'static str,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Status> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| Status::internal(format!("{what} panicked: {e}")))
}

/// `GetNodeInfo`, the one function both encodings call. `vms` is the composed
/// VM port (ADR-0040 A5): the measured provider reports come through it, never
/// the `delonix-vm` adapter.
pub async fn node_info(
    vms: Arc<dyn crate::vms::VmPort>,
    net: Arc<dyn crate::net_ops::NetworkOps>,
    providers: Arc<dyn crate::provider_reports::ProviderReports>,
    linux: Arc<dyn crate::linux_lifecycle::LinuxLifecycle>,
) -> Result<NodeInfo, Status> {
    blocking("node info", move || {
        node::node_info(
            &crate::providers::measured_reports(&*vms, &*net, &*providers),
            &*linux,
        )
    })
    .await
}

/// `GetHealth`, the one function both encodings call.
pub async fn health(
    vms: Arc<dyn crate::vms::VmPort>,
    net: Arc<dyn crate::net_ops::NetworkOps>,
    providers: Arc<dyn crate::provider_reports::ProviderReports>,
    linux: Arc<dyn crate::linux_lifecycle::LinuxLifecycle>,
) -> Result<Health, Status> {
    blocking("health", move || {
        node::health(
            &crate::providers::measured_reports(&*vms, &*net, &*providers),
            &*linux,
        )
    })
    .await
}

/// `GetCapacity`, the one function both encodings call.
pub async fn capacity(
    vms: Arc<dyn crate::vms::VmPort>,
    store: Arc<dyn crate::store::StorePort>,
    linux: Arc<dyn crate::linux_lifecycle::LinuxLifecycle>,
) -> Result<Capacity, Status> {
    blocking("capacity", move || node::capacity(&*vms, &*store, &*linux)).await
}

/// `ListProviders`, the one function both encodings call (ADR-0050 D5).
/// `kind` empty = all; a kind the catalog does not have is `INVALID_ARGUMENT`
/// (the same refusal `provider ls --kind ceph` gives), not an empty list.
pub async fn list_providers(
    vms: Arc<dyn crate::vms::VmPort>,
    net: Arc<dyn crate::net_ops::NetworkOps>,
    providers: Arc<dyn crate::provider_reports::ProviderReports>,
    req: ListProvidersRequest,
) -> Result<ListProvidersResponse, Status> {
    let kind = match req.kind.as_str() {
        "" => None,
        k @ ("compute" | "network" | "storage" | "image" | "gateway") => Some(k.to_string()),
        other => {
            return Err(Status::invalid_argument(format!(
                "unknown provider kind '{other}': compute, network, storage, image or gateway"
            )))
        }
    };
    // The reports probe the host (binaries on the PATH, /dev/kvm, the holder):
    // blocking work, off the runtime's workers.
    let reports = tokio::task::spawn_blocking(move || {
        crate::providers::measured_reports(&*vms, &*net, &*providers)
    })
    .await
    .map_err(|e| Status::internal(format!("provider probe panicked: {e}")))?;
    let this = match &kind {
        Some(k) => format!("/v1/providers?kind={k}"),
        None => "/v1/providers".to_string(),
    };
    Ok(ListProvidersResponse {
        links: vec![node::link("self", &this), node::link("root", "/v1")],
        providers: reports
            .iter()
            .filter(|r| kind.as_deref().is_none_or(|k| r.kind.as_str() == k))
            .map(crate::providers::provider_info)
            .collect(),
    })
}

#[tonic::async_trait]
impl NodeService for NodeApi {
    async fn get_api_root(
        &self,
        _req: Request<GetApiRootRequest>,
    ) -> Result<Response<ApiRoot>, Status> {
        Ok(Response::new(node::api_root()))
    }

    async fn get_node_info(
        &self,
        _req: Request<GetNodeInfoRequest>,
    ) -> Result<Response<NodeInfo>, Status> {
        Ok(Response::new(
            node_info(
                Arc::clone(&self.vms),
                Arc::clone(&self.net),
                Arc::clone(&self.providers),
                Arc::clone(&self.linux),
            )
            .await?,
        ))
    }

    async fn get_health(
        &self,
        _req: Request<GetHealthRequest>,
    ) -> Result<Response<Health>, Status> {
        Ok(Response::new(
            health(
                Arc::clone(&self.vms),
                Arc::clone(&self.net),
                Arc::clone(&self.providers),
                Arc::clone(&self.linux),
            )
            .await?,
        ))
    }

    async fn get_capacity(
        &self,
        _req: Request<GetCapacityRequest>,
    ) -> Result<Response<Capacity>, Status> {
        Ok(Response::new(
            capacity(
                Arc::clone(&self.vms),
                Arc::clone(&self.store),
                Arc::clone(&self.linux),
            )
            .await?,
        ))
    }

    async fn list_providers(
        &self,
        req: Request<ListProvidersRequest>,
    ) -> Result<Response<ListProvidersResponse>, Status> {
        Ok(Response::new(
            list_providers(
                Arc::clone(&self.vms),
                Arc::clone(&self.net),
                Arc::clone(&self.providers),
                req.into_inner(),
            )
            .await?,
        ))
    }

    type WatchEventsStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<Event, Status>> + Send + 'static>>;

    async fn watch_events(
        &self,
        req: Request<WatchEventsRequest>,
    ) -> Result<Response<Self::WatchEventsStream>, Status> {
        let req = req.into_inner();
        // The replay reads files: off the workers, and the polling task it
        // spawns lands on the runtime (a blocking-pool thread is inside it).
        let stream = blocking("events watch", move || {
            crate::events::watch(node::state_root(), req, crate::events::POLL)
        })
        .await??;
        Ok(Response::new(Box::pin(stream)))
    }
}

/// A served service's RPC that is not served yet — the same answer the REST
/// encoding gives for a whole service that is not ([`crate::transcode::not_served`]).
fn unserved(service: &str, rpc: &str) -> Status {
    Status::unimplemented(format!(
        "{service}.{rpc} is in the contract and this engine does not serve it yet"
    ))
}

#[tonic::async_trait]
impl NetworkService for NodeApi {
    async fn get_network(
        &self,
        req: Request<GetNetworkRequest>,
    ) -> Result<Response<Network>, Status> {
        let req = req.into_inner();
        let net = Arc::clone(&self.net);
        blocking("network", move || {
            crate::networks::get_in(&node::state_root(), net.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn list_networks(
        &self,
        req: Request<ListNetworksRequest>,
    ) -> Result<Response<ListNetworksResponse>, Status> {
        let req = req.into_inner();
        let net = Arc::clone(&self.net);
        blocking("networks", move || {
            crate::networks::list_in(&node::state_root(), net.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn create_network(
        &self,
        req: Request<CreateNetworkRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let net = Arc::clone(&self.net);
        blocking("network create", move || {
            crate::network_ops::create_in(&node::state_root(), net.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn delete_network(
        &self,
        req: Request<DeleteNetworkRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let net = Arc::clone(&self.net);
        blocking("network delete", move || {
            crate::network_ops::delete_in(&node::state_root(), store.as_ref(), net.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn connect_container(
        &self,
        req: Request<ConnectContainerRequest>,
    ) -> Result<Response<Container>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("network connect", move || {
            crate::network_ops::connect_in(&node::state_root(), store.as_ref(), linux.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn disconnect_container(
        &self,
        req: Request<DisconnectContainerRequest>,
    ) -> Result<Response<Container>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("network disconnect", move || {
            crate::network_ops::disconnect_in(
                &node::state_root(),
                store.as_ref(),
                linux.as_ref(),
                &req,
            )
        })
        .await?
        .map(Response::new)
    }
}

#[tonic::async_trait]
impl VolumeService for NodeApi {
    async fn get_volume(&self, req: Request<GetVolumeRequest>) -> Result<Response<Volume>, Status> {
        let req = req.into_inner();
        blocking("volume", move || {
            crate::volumes::get_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn list_volumes(
        &self,
        req: Request<ListVolumesRequest>,
    ) -> Result<Response<ListVolumesResponse>, Status> {
        let req = req.into_inner();
        blocking("volumes", move || {
            crate::volumes::list_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn create_volume(
        &self,
        req: Request<CreateVolumeRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        blocking("volume create", move || {
            crate::volume_ops::create_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn delete_volume(
        &self,
        req: Request<DeleteVolumeRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        blocking("volume delete", move || {
            crate::volume_ops::delete_in(&node::state_root(), store.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }
}

#[tonic::async_trait]
impl StorageService for NodeApi {
    async fn get_ceph_status(
        &self,
        req: Request<GetCephStatusRequest>,
    ) -> Result<Response<CephStatus>, Status> {
        let req = req.into_inner();
        blocking("ceph status", move || crate::storage::ceph_status(&req))
            .await?
            .map(Response::new)
    }

    async fn list_rbd_images(
        &self,
        req: Request<ListRbdImagesRequest>,
    ) -> Result<Response<ListRbdImagesResponse>, Status> {
        let req = req.into_inner();
        blocking("rbd images", move || crate::storage::list_rbd_images(&req))
            .await?
            .map(Response::new)
    }

    async fn list_cephfs_subvolumes(
        &self,
        req: Request<ListCephfsSubvolumesRequest>,
    ) -> Result<Response<ListCephfsSubvolumesResponse>, Status> {
        let req = req.into_inner();
        blocking("cephfs subvolumes", move || {
            crate::storage::list_cephfs_subvolumes(&req)
        })
        .await?
        .map(Response::new)
    }

    async fn list_object_store_buckets(
        &self,
        req: Request<ListObjectStoreBucketsRequest>,
    ) -> Result<Response<ListObjectStoreBucketsResponse>, Status> {
        let req = req.into_inner();
        blocking("rgw buckets", move || crate::storage::list_buckets(&req))
            .await?
            .map(Response::new)
    }

    async fn get_object_store_bucket(
        &self,
        req: Request<GetObjectStoreBucketRequest>,
    ) -> Result<Response<ObjectStoreBucket>, Status> {
        let req = req.into_inner();
        blocking("rgw bucket", move || crate::storage::get_bucket(&req))
            .await?
            .map(Response::new)
    }

    async fn create_object_store_user(
        &self,
        req: Request<CreateObjectStoreUserRequest>,
    ) -> Result<Response<ObjectStoreUser>, Status> {
        let req = req.into_inner();
        blocking("rgw user", move || crate::storage::create_user(&req))
            .await?
            .map(Response::new)
    }
}

#[tonic::async_trait]
impl ImageService for NodeApi {
    // Read half served from the OCI image store, in-process (no re-exec).
    async fn get_image(&self, req: Request<GetImageRequest>) -> Result<Response<Image>, Status> {
        let req = req.into_inner();
        let images = Arc::clone(&self.images);
        blocking("image", move || {
            crate::images::get_in(&node::state_root(), images.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn list_images(
        &self,
        req: Request<ListImagesRequest>,
    ) -> Result<Response<ListImagesResponse>, Status> {
        let req = req.into_inner();
        let images = Arc::clone(&self.images);
        blocking("images", move || {
            crate::images::list_in(&node::state_root(), images.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    // Pull (re-exec of `delonix pull`) and Delete (in-process `ImageStore`
    // remove) are served. Push/build/scan stay unserved (501, DX-6001).
    async fn pull_image(
        &self,
        req: Request<PullImageRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        blocking("image pull", move || {
            crate::image_ops::pull_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn delete_image(
        &self,
        req: Request<DeleteImageRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let images = Arc::clone(&self.images);
        blocking("image delete", move || {
            crate::image_ops::delete_in(&node::state_root(), images.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn push_image(
        &self,
        req: Request<PushImageRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        blocking("image push", move || {
            crate::image_ops::push_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn build_image(
        &self,
        req: Request<BuildImageRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        blocking("image build", move || {
            crate::image_ops::build_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn scan_image(
        &self,
        req: Request<ScanImageRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        blocking("image scan", move || {
            crate::image_ops::scan_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }
}

#[tonic::async_trait]
impl PodService for NodeApi {
    // Read half derived from the container records (a pod is its members).
    async fn get_pod(&self, req: Request<GetPodRequest>) -> Result<Response<Pod>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("pod", move || {
            crate::pods::get_in(&node::state_root(), store.as_ref(), linux.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn list_pods(
        &self,
        req: Request<ListPodsRequest>,
    ) -> Result<Response<ListPodsResponse>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("pods", move || {
            crate::pods::list_in(&node::state_root(), store.as_ref(), linux.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    // Create (from a `kind: Pod` manifest) and Delete (removing every member)
    // re-exec the CLI that owns the pod construction — the same fork-safe seam
    // Start/Exec use. Both answer with a terminal Operation.
    async fn create_pod(
        &self,
        req: Request<CreatePodRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        blocking("pod create", move || {
            crate::pod_ops::create_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn delete_pod(
        &self,
        req: Request<DeletePodRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        blocking("pod delete", move || {
            crate::pod_ops::delete_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }
}

#[tonic::async_trait]
impl StackService for NodeApi {
    async fn plan_stack(
        &self,
        req: Request<PlanStackRequest>,
    ) -> Result<Response<StackPlan>, Status> {
        let req = req.into_inner();
        blocking("stack plan", move || {
            crate::stacks::plan_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn apply_stack(
        &self,
        req: Request<ApplyStackRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        blocking("stack apply", move || {
            crate::stacks::apply_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn destroy_stack(
        &self,
        req: Request<DestroyStackRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        blocking("stack destroy", move || {
            crate::stacks::destroy_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }
}

#[tonic::async_trait]
impl VirtualMachineService for NodeApi {
    async fn get_virtual_machine(
        &self,
        req: Request<GetVirtualMachineRequest>,
    ) -> Result<Response<VirtualMachine>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machine", move || crate::vms::get_in(&*vms, &req))
            .await?
            .map(Response::new)
    }

    async fn list_virtual_machines(
        &self,
        req: Request<ListVirtualMachinesRequest>,
    ) -> Result<Response<ListVirtualMachinesResponse>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machines", move || {
            crate::vms::list_in(&node::state_root(), &*vms, &req)
        })
        .await?
        .map(Response::new)
    }

    async fn create_virtual_machine(
        &self,
        req: Request<CreateVirtualMachineRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        let store = Arc::clone(&self.store);
        let net = Arc::clone(&self.net);
        blocking("virtual machine create", move || {
            crate::vm_ops::create_in(
                &node::state_root(),
                store.as_ref(),
                &*vms,
                net.as_ref(),
                &req,
            )
        })
        .await?
        .map(Response::new)
    }

    async fn delete_virtual_machine(
        &self,
        req: Request<DeleteVirtualMachineRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machine delete", move || {
            crate::vm_ops::delete_in(&node::state_root(), &*vms, &req)
        })
        .await?
        .map(Response::new)
    }

    async fn start_virtual_machine(
        &self,
        req: Request<StartVirtualMachineRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machine start", move || {
            crate::vm_ops::start_in(&node::state_root(), &*vms, &req)
        })
        .await?
        .map(Response::new)
    }

    async fn stop_virtual_machine(
        &self,
        req: Request<StopVirtualMachineRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machine stop", move || {
            crate::vm_ops::stop_in(&node::state_root(), &*vms, &req)
        })
        .await?
        .map(Response::new)
    }

    async fn pause_virtual_machine(
        &self,
        req: Request<PauseVirtualMachineRequest>,
    ) -> Result<Response<VirtualMachine>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machine pause", move || {
            crate::vm_ops::pause_in(&*vms, &req)
        })
        .await?
        .map(Response::new)
    }

    async fn resume_virtual_machine(
        &self,
        req: Request<ResumeVirtualMachineRequest>,
    ) -> Result<Response<VirtualMachine>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machine resume", move || {
            crate::vm_ops::resume_in(&*vms, &req)
        })
        .await?
        .map(Response::new)
    }

    async fn create_snapshot(
        &self,
        req: Request<CreateSnapshotRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machine snapshot create", move || {
            crate::vm_ops::create_snapshot_in(&node::state_root(), &*vms, &req)
        })
        .await?
        .map(Response::new)
    }

    async fn list_snapshots(
        &self,
        req: Request<ListSnapshotsRequest>,
    ) -> Result<Response<ListSnapshotsResponse>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machine snapshots", move || {
            crate::vm_ops::list_snapshots_in(&*vms, &req)
        })
        .await?
        .map(Response::new)
    }

    async fn restore_snapshot(
        &self,
        req: Request<RestoreSnapshotRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machine snapshot restore", move || {
            crate::vm_ops::restore_snapshot_in(&node::state_root(), &*vms, &req)
        })
        .await?
        .map(Response::new)
    }

    async fn delete_snapshot(
        &self,
        req: Request<DeleteSnapshotRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let vms = Arc::clone(&self.vms);
        blocking("virtual machine snapshot delete", move || {
            crate::vm_ops::delete_snapshot_in(&node::state_root(), &*vms, &req)
        })
        .await?
        .map(Response::new)
    }

    type ConsoleStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<ConsoleResponse, Status>> + Send + 'static>>;

    async fn console(
        &self,
        _req: Request<tonic::Streaming<ConsoleRequest>>,
    ) -> Result<Response<Self::ConsoleStream>, Status> {
        // A serial console is a live byte pipe into a running guest — the
        // same family as `ContainerService.Exec`, which this socket does not
        // serve at all yet. Named so a caller does not read a generic
        // "service not found": the method exists in the contract, this build
        // has nowhere to plumb it to.
        Err(unserved("VirtualMachineService", "Console"))
    }
}

#[tonic::async_trait]
impl OperationService for NodeApi {
    async fn get_operation(
        &self,
        req: Request<GetOperationRequest>,
    ) -> Result<Response<Operation>, Status> {
        let id = req.into_inner().id;
        blocking("operation", move || {
            crate::operations::get_in(&node::state_root(), &id)
        })
        .await?
        .map(Response::new)
    }

    async fn list_operations(
        &self,
        req: Request<ListOperationsRequest>,
    ) -> Result<Response<ListOperationsResponse>, Status> {
        let req = req.into_inner();
        blocking("operations", move || {
            crate::operations::list_in(&node::state_root(), &req)
        })
        .await?
        .map(Response::new)
    }

    type WatchOperationStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<Operation, Status>> + Send + 'static>>;

    async fn watch_operation(
        &self,
        req: Request<WatchOperationRequest>,
    ) -> Result<Response<Self::WatchOperationStream>, Status> {
        let id = req.into_inner().id;
        let stream = blocking("operation watch", move || {
            // The first read is blocking file I/O. A blocking-pool thread is
            // inside the runtime, so the polling task `watch` spawns lands on
            // it, and reads off the workers in turn.
            crate::operations::watch(node::state_root(), id, crate::operations::WATCH_POLL)
        })
        .await??;
        Ok(Response::new(Box::pin(stream)))
    }

    async fn cancel_operation(
        &self,
        req: Request<CancelOperationRequest>,
    ) -> Result<Response<Operation>, Status> {
        let id = req.into_inner().id;
        blocking("operation cancel", move || {
            crate::operations::cancel_in(&node::state_root(), &id)
        })
        .await?
        .map(Response::new)
    }
}

#[tonic::async_trait]
impl ContainerService for NodeApi {
    async fn get_container(
        &self,
        req: Request<GetContainerRequest>,
    ) -> Result<Response<Container>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("container", move || {
            crate::containers::get_in(&node::state_root(), store.as_ref(), linux.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn list_containers(
        &self,
        req: Request<ListContainersRequest>,
    ) -> Result<Response<ListContainersResponse>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("containers", move || {
            crate::containers::list_in(&node::state_root(), store.as_ref(), linux.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    // ---- lifecycle write-path + stats + logs + resize. `Exec` is the one the
    // contract still carries unserved here (its own pass).

    async fn create_container(
        &self,
        req: Request<CreateContainerRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let run_resolver = Arc::clone(&self.run_resolver);
        let net = Arc::clone(&self.net);
        blocking("container create", move || {
            crate::container_launch::create_in(
                &node::state_root(),
                store.as_ref(),
                run_resolver.as_ref(),
                net.as_ref(),
                &req,
            )
        })
        .await?
        .map(Response::new)
    }

    async fn update_container(
        &self,
        req: Request<UpdateContainerRequest>,
    ) -> Result<Response<Container>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("container update", move || {
            crate::container_update::update_in(
                &node::state_root(),
                store.as_ref(),
                linux.as_ref(),
                &req,
            )
        })
        .await?
        .map(Response::new)
    }

    async fn delete_container(
        &self,
        req: Request<DeleteContainerRequest>,
    ) -> Result<Response<Operation>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("container delete", move || {
            crate::container_ops::delete_in(
                &node::state_root(),
                store.as_ref(),
                linux.as_ref(),
                &req,
            )
        })
        .await?
        .map(Response::new)
    }

    async fn start_container(
        &self,
        req: Request<StartContainerRequest>,
    ) -> Result<Response<Container>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("container start", move || {
            crate::container_launch::start_in(
                &node::state_root(),
                store.as_ref(),
                linux.as_ref(),
                &req,
            )
        })
        .await?
        .map(Response::new)
    }

    async fn stop_container(
        &self,
        req: Request<StopContainerRequest>,
    ) -> Result<Response<Container>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("container stop", move || {
            crate::container_ops::stop_in(&node::state_root(), store.as_ref(), linux.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn kill_container(
        &self,
        req: Request<KillContainerRequest>,
    ) -> Result<Response<Container>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("container kill", move || {
            crate::container_ops::kill_in(&node::state_root(), store.as_ref(), linux.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    async fn wait_container(
        &self,
        req: Request<WaitContainerRequest>,
    ) -> Result<Response<WaitContainerResponse>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("container wait", move || {
            crate::container_launch::wait_in(
                &node::state_root(),
                store.as_ref(),
                linux.as_ref(),
                &req,
            )
        })
        .await?
        .map(Response::new)
    }

    async fn stats(&self, req: Request<StatsRequest>) -> Result<Response<ContainerStats>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        blocking("container stats", move || {
            crate::container_ops::stats_in(&node::state_root(), store.as_ref(), linux.as_ref(), &req)
        })
        .await?
        .map(Response::new)
    }

    type ExecStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<ExecResponse, Status>> + Send + 'static>>;

    async fn exec(
        &self,
        req: Request<tonic::Streaming<ExecRequest>>,
    ) -> Result<Response<Self::ExecStream>, Status> {
        let stream = crate::container_exec::serve(
            node::state_root(),
            Arc::clone(&self.store),
            Arc::clone(&self.linux),
            req.into_inner(),
        )
        .await?;
        Ok(Response::new(Box::pin(stream)))
    }

    type LogsStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<LogChunk, Status>> + Send + 'static>>;

    async fn logs(&self, req: Request<LogsRequest>) -> Result<Response<Self::LogsStream>, Status> {
        let req = req.into_inner();
        let store = Arc::clone(&self.store);
        let linux = Arc::clone(&self.linux);
        // The backlog read is file I/O; the follow task it spawns lands on the
        // runtime (a blocking-pool thread per poll), as `WatchEvents` does.
        let stream = blocking("container logs", move || {
            crate::container_logs::stream(
                node::state_root(),
                store,
                linux,
                req,
                crate::container_logs::POLL,
            )
        })
        .await??;
        Ok(Response::new(Box::pin(stream)))
    }
}

/// The router both transports share. The contract's REST routes are not
/// listed here: they are resolved against the table `build.rs` generates from
/// the `google.api.http` annotations ([`crate::transcode`]), in the fallback —
/// a template such as `{name}:start` is not something an axum route spells.
/// The gRPC service is merged in (tonic's routes are an axum router
/// underneath). Exposed for the in-process tests, which drive it with
/// `tower::ServiceExt::oneshot`.
pub fn router() -> axum::Router {
    // The convenience builder: a server with an empty VM port, for tests and any
    // caller that does not exercise the VM reads. Production mounts the engine
    // through [`router_with`] (see `serve`).
    router_with(
        Arc::new(NoVms),
        Arc::new(crate::store::NoStore),
        Arc::new(crate::run_resolver::NoResolver),
        Arc::new(crate::net_ops::NoNet),
        Arc::new(crate::image_repo::NoImages),
        Arc::new(crate::provider_reports::NoProviderReports),
        Arc::new(crate::linux_lifecycle::NoLinux),
    )
}

/// A [`crate::vms::VmPort`] with no VMs — the empty backing [`router`] hands the
/// server when no engine is injected. The VM read path is proven end-to-end
/// through the engine in `delonix-node-api-bin`.
struct NoVms;
impl crate::vms::VmPort for NoVms {
    fn list(&self) -> Result<Vec<delonix_compute::Vm>, delonix_model::Error> {
        Ok(Vec::new())
    }
    fn status(&self, name: &str) -> Result<delonix_compute::Vm, delonix_model::Error> {
        Err(delonix_model::Error::VmNotFound(name.to_string()))
    }
    fn exists(&self, _name: &str) -> bool {
        false
    }
    // No VM exists, so every mutation on a named VM is NotFound; snapshots of a
    // non-existent VM is the empty list, never an error.
    fn start(&self, name: &str) -> Result<(), delonix_model::Error> {
        Err(delonix_model::Error::VmNotFound(name.to_string()))
    }
    fn stop(&self, name: &str) -> Result<(), delonix_model::Error> {
        Err(delonix_model::Error::VmNotFound(name.to_string()))
    }
    fn pause(&self, name: &str) -> Result<(), delonix_model::Error> {
        Err(delonix_model::Error::VmNotFound(name.to_string()))
    }
    fn unpause(&self, name: &str) -> Result<(), delonix_model::Error> {
        Err(delonix_model::Error::VmNotFound(name.to_string()))
    }
    fn snapshots(&self, _name: &str) -> Result<Vec<String>, delonix_model::Error> {
        Ok(Vec::new())
    }
    fn snapshot(&self, name: &str, _snap: &str) -> Result<(), delonix_model::Error> {
        Err(delonix_model::Error::VmNotFound(name.to_string()))
    }
    fn restore(&self, name: &str, _snap: &str) -> Result<(), delonix_model::Error> {
        Err(delonix_model::Error::VmNotFound(name.to_string()))
    }
    fn delete_snapshot(&self, name: &str, _snap: &str) -> Result<(), delonix_model::Error> {
        Err(delonix_model::Error::VmNotFound(name.to_string()))
    }
    fn destroy(&self, name: &str, _force: bool, _purge: bool) -> Result<(), delonix_model::Error> {
        Err(delonix_model::Error::VmNotFound(name.to_string()))
    }
    // The write/resolution half: an empty backing has no engine to compose a
    // VM with. A create never reaches here in practice — `config_from_spec`
    // refuses an empty `spec.image` first, and the router's own tests only
    // drive refusals — but the port is total, so each answers the safe nothing.
    fn create(
        &self,
        _cfg: &delonix_compute::vm_backend::VmConfig,
    ) -> Result<(), delonix_model::Error> {
        Err(delonix_model::Error::Unavailable(
            "no VM engine is composed on this server (serve with an engine-backed port)".into(),
        ))
    }
    fn image_defaults(&self, _reference: &str) -> Option<delonix_compute::ports::ImageDefaults> {
        None
    }
    fn store_image_disk(&self, reference: &str) -> String {
        reference.to_string()
    }
    fn refuse_undeclared_network(
        &self,
        _cfg: &delonix_compute::vm_backend::VmConfig,
    ) -> Result<(), delonix_model::Error> {
        Ok(())
    }
    fn ip_is_predicted(&self, _vm: &delonix_compute::Vm) -> bool {
        false
    }
    fn provider_reports(&self) -> Vec<delonix_compute::capability::ProviderReport> {
        Vec::new()
    }
    fn declared_vm_reports(&self) -> Vec<delonix_compute::capability::ProviderReport> {
        Vec::new()
    }
}

/// Builds the router with the VM port the composition injects (ADR-0040 A5).
/// `delonix-node-api-bin` passes an engine-backed [`crate::vms::VmPort`];
/// [`router`] passes an empty one.
pub fn router_with(
    vms: Arc<dyn crate::vms::VmPort>,
    store: Arc<dyn crate::store::StorePort>,
    run_resolver: Arc<dyn crate::run_resolver::RunResolver>,
    net: Arc<dyn crate::net_ops::NetworkOps>,
    images: Arc<dyn crate::image_repo::ImageRepo>,
    providers: Arc<dyn crate::provider_reports::ProviderReports>,
    linux: Arc<dyn crate::linux_lifecycle::LinuxLifecycle>,
) -> axum::Router {
    use tonic::server::NamedService;
    let api = NodeApi {
        vms,
        store,
        run_resolver,
        net,
        images,
        providers,
        linux,
    };
    // The gRPC service is mounted the way tonic's own `Routes` mounts it — one
    // wildcard route under the service name. Not through `Routes::into_axum_router`,
    // whose fallback answers EVERY unknown path with HTTP 200 + `grpc-status: 12`:
    // measured in the battery, `GET /v1/node` came back 200 with an empty body,
    // which a REST client reads as "served, nothing there". The fallback below
    // keeps that answer for gRPC callers and gives HTTP callers a 404.
    axum::Router::new()
        .route("/openapi.json", axum::routing::get(openapi_json))
        .route("/docs", axum::routing::get(crate::docs::swagger))
        .route("/redoc", axum::routing::get(crate::docs::redoc))
        .route("/docs/assets/:name", axum::routing::get(crate::docs::asset))
        .route_service(
            &format!("/{}/*rest", NodeServiceServer::<NodeApi>::NAME),
            NodeServiceServer::new(api.clone()),
        )
        .route_service(
            &format!("/{}/*rest", NetworkServiceServer::<NodeApi>::NAME),
            NetworkServiceServer::new(api.clone()),
        )
        .route_service(
            &format!("/{}/*rest", VolumeServiceServer::<NodeApi>::NAME),
            VolumeServiceServer::new(api.clone()),
        )
        .route_service(
            &format!("/{}/*rest", ContainerServiceServer::<NodeApi>::NAME),
            ContainerServiceServer::new(api.clone()),
        )
        .route_service(
            &format!("/{}/*rest", OperationServiceServer::<NodeApi>::NAME),
            OperationServiceServer::new(api.clone()),
        )
        .route_service(
            &format!("/{}/*rest", VirtualMachineServiceServer::<NodeApi>::NAME),
            VirtualMachineServiceServer::new(api.clone()),
        )
        .route_service(
            &format!("/{}/*rest", StorageServiceServer::<NodeApi>::NAME),
            StorageServiceServer::new(api.clone()),
        )
        .route_service(
            &format!("/{}/*rest", ImageServiceServer::<NodeApi>::NAME),
            ImageServiceServer::new(api.clone()),
        )
        .route_service(
            &format!("/{}/*rest", PodServiceServer::<NodeApi>::NAME),
            PodServiceServer::new(api.clone()),
        )
        .route_service(
            &format!("/{}/*rest", StackServiceServer::<NodeApi>::NAME),
            StackServiceServer::new(api.clone()),
        )
        .fallback({
            // The REST transcode path needs the same `NodeApi` (with its VM port)
            // the gRPC services hold; carry it into the fallback by capture.
            let api = api.clone();
            move |req: axum::extract::Request| {
                let api = api.clone();
                async move { fallback(api, req).await }
            }
        })
        .method_not_allowed_fallback(|req: axum::extract::Request| async move {
            let path = req.uri().path().to_string();
            method_not_allowed(req.method().as_str(), &path, &["GET"])
        })
}

/// The largest request body the REST encoding reads.
const MAX_BODY: usize = 1024 * 1024;

/// A request for one of the contract's REST routes: bound, dispatched to the
/// service method the gRPC encoding calls, answered as JSON. A route of a
/// service this engine does not serve yet is 501 (DX-6001) — in the contract,
/// not here yet — which is a different answer from a path the contract does
/// not have (404).
async fn rest(
    api: &NodeApi,
    route: &'static crate::transcode::Route,
    vars: Vec<(&'static str, String)>,
    req: axum::extract::Request,
) -> axum::response::Response {
    use crate::transcode;
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(str::to_string);
    let if_none_match = req
        .headers()
        .get(hyper::header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    // Read before the body is taken: a borrow of the request held across an
    // await would make this future not `Send`.
    let (idempotency_key, if_match) = {
        let header = |name: &str| {
            req.headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        (header("idempotency-key"), header("if-match"))
    };
    let answer = async {
        let body = axum::body::to_bytes(req.into_body(), MAX_BODY)
            .await
            .map_err(|_| {
                Status::invalid_argument(format!(
                    "{}: the request body is larger than {MAX_BODY} bytes",
                    route.rpc
                ))
            })?;
        let mut input = transcode::bind(route, &vars, query.as_deref(), &body)?;
        if MUTATIONS.contains(&route.rpc) {
            from_header(&mut input, "request_id", "Idempotency-Key", idempotency_key)?;
            from_header(
                &mut input,
                "etag",
                "If-Match",
                if_match.map(|v| v.trim_start_matches("W/").trim_matches('"').to_string()),
            )?;
        }
        match route.service {
            "NodeService" => transcode::dispatch_node_service(api, route.rpc, input).await,
            "NetworkService" => transcode::dispatch_network_service(api, route.rpc, input).await,
            "VolumeService" => transcode::dispatch_volume_service(api, route.rpc, input).await,
            "ContainerService" => {
                transcode::dispatch_container_service(api, route.rpc, input).await
            }
            "OperationService" => {
                transcode::dispatch_operation_service(api, route.rpc, input).await
            }
            "VirtualMachineService" => {
                transcode::dispatch_virtual_machine_service(api, route.rpc, input).await
            }
            "StorageService" => transcode::dispatch_storage_service(api, route.rpc, input).await,
            "ImageService" => transcode::dispatch_image_service(api, route.rpc, input).await,
            "PodService" => transcode::dispatch_pod_service(api, route.rpc, input).await,
            "StackService" => transcode::dispatch_stack_service(api, route.rpc, input).await,
            _ => Err(transcode::not_served(route)),
        }
    };
    match answer.await {
        Ok(body) if MUTATIONS.contains(&route.rpc) => acknowledged(with_link_header_of(body)),
        Ok(body) => with_etag(with_link_header_of(body), if_none_match.as_deref()),
        Err(status) => problem(status, &path),
    }
}

/// The RPCs that answer an `Operation` and read the two request headers of
/// ADR-0042 D2: `Idempotency-Key` (the request's `request_id`) and `If-Match`
/// (its `etag`). A header on any other route is not read, and `If-Match` on a
/// request with no `etag` field is refused.
const MUTATIONS: &[&str] = &[
    "CreateNetwork",
    "DeleteNetwork",
    "CreateVolume",
    "DeleteVolume",
    "CreateVirtualMachine",
    "DeleteVirtualMachine",
    "StartVirtualMachine",
    "StopVirtualMachine",
    "CreateSnapshot",
    "RestoreSnapshot",
    "DeleteSnapshot",
    "CreateContainer",
    "DeleteContainer",
];

/// Puts a header's value in the request field it stands for. The same value
/// in both places is fine; two different ones are refused — which of them the
/// server obeyed would be a guess.
fn from_header(
    input: &mut serde_json::Value,
    field: &str,
    header: &str,
    value: Option<String>,
) -> Result<(), Status> {
    let (Some(value), Some(msg)) = (value, input.as_object_mut()) else {
        return Ok(());
    };
    if header == "If-Match" && value == "*" {
        return Ok(());
    }
    match msg.get(field).and_then(|v| v.as_str()) {
        Some(sent) if !sent.is_empty() && sent != value => Err(Status::invalid_argument(format!(
            "the {header} header ('{value}') and the request's {field} ('{sent}') disagree"
        ))),
        _ => {
            msg.insert(field.to_string(), value.into());
            Ok(())
        }
    }
}

/// The answer to a mutation (ADR-0042 D2): the `Operation`, with `Location`
/// naming it. `202 Accepted` while it runs; `200` once it has ended — the
/// body says how.
fn acknowledged(
    (body, mut res): (serde_json::Value, axum::response::Response),
) -> axum::response::Response {
    let running = matches!(
        body.get("state").and_then(|s| s.as_str()),
        Some("OPERATION_STATE_PENDING" | "OPERATION_STATE_RUNNING")
    );
    if running {
        *res.status_mut() = hyper::StatusCode::ACCEPTED;
    }
    if let Some(v) = body
        .get("id")
        .and_then(|i| i.as_str())
        .and_then(|id| hyper::header::HeaderValue::from_str(&format!("/v1/operations/{id}")).ok())
    {
        res.headers_mut().insert(hyper::header::LOCATION, v);
    }
    res
}

/// ADR-0042 D2, concurrency: a resource's `meta.etag` is its `ETag` header,
/// and a `GET` whose `If-None-Match` names that version is `304 Not Modified`
/// with no body. Only the REST routes come through here, and of those only
/// `GET` answers a resource — a mutation answers an `Operation`, which has no
/// `meta`.
fn with_etag(
    (body, mut res): (serde_json::Value, axum::response::Response),
    if_none_match: Option<&str>,
) -> axum::response::Response {
    let Some(etag) = body
        .pointer("/meta/etag")
        .and_then(|v| v.as_str())
        .filter(|e| !e.is_empty())
    else {
        return res;
    };
    let quoted = format!("\"{etag}\"");
    let Ok(value) = hyper::header::HeaderValue::from_str(&quoted) else {
        return res;
    };
    let unchanged = if_none_match.is_some_and(|sent| {
        sent.split(',')
            .map(|t| t.trim().trim_start_matches("W/"))
            .any(|t| t == "*" || t == quoted)
    });
    if unchanged {
        res = hyper::StatusCode::NOT_MODIFIED.into_response();
    }
    res.headers_mut().insert(hyper::header::ETAG, value);
    res
}

/// Everything the router has no route for. One of the contract's REST routes
/// is served ([`rest`]); a contract path on another method is 405 with the
/// methods the contract maps; and an unknown path gets: a gRPC caller (content-type `application/grpc…`)
/// the wire-level UNIMPLEMENTED tonic would give; anyone else a 404 carrying a
/// problem document (DX-4001, `application/problem+json`) — never a 200 with
/// nothing in it.
async fn fallback(api: NodeApi, req: axum::extract::Request) -> axum::response::Response {
    use crate::transcode::Resolved;
    match crate::transcode::resolve(req.method().as_str(), req.uri().path()) {
        Resolved::Route(route, vars) => return rest(&api, route, vars, req).await,
        Resolved::OtherMethods(allow) => {
            return method_not_allowed(req.method().as_str(), req.uri().path(), &allow)
        }
        Resolved::Unknown => {}
    }
    let is_grpc = req
        .headers()
        .get(hyper::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/grpc"));
    if is_grpc {
        return hyper::Response::builder()
            .status(hyper::StatusCode::OK)
            .header("grpc-status", "12")
            .header(hyper::header::CONTENT_TYPE, "application/grpc")
            .body(axum::body::Body::empty())
            .expect("static response");
    }
    let path = req.uri().path().to_string();
    problem(
        no_route(format!("route {} {path} on the node API", req.method())),
        &path,
    )
}

/// A message's JSON with the RFC 8288 `Link` header mirroring its own `links`
/// (ADR-0042 D2) — read from the body being sent, so the header and the body
/// cannot disagree.
fn with_link_header_of(body: serde_json::Value) -> (serde_json::Value, axum::response::Response) {
    let header = link_header(&body);
    let mut res = (hyper::StatusCode::OK, axum::Json(&body)).into_response();
    if let Some(v) = header.and_then(|h| hyper::header::HeaderValue::from_str(&h).ok()) {
        res.headers_mut().insert(hyper::header::LINK, v);
    }
    (body, res)
}

/// `</v1/node>; rel="self", </v1>; rel="root"` from a message's `links`;
/// `None` when it has none.
pub fn link_header(body: &serde_json::Value) -> Option<String> {
    let links = body.get("links")?.as_array()?;
    let parts: Vec<String> = links
        .iter()
        .filter_map(|l| {
            let href = l.get("href")?.as_str()?;
            let rel = l.get("rel")?.as_str()?;
            Some(format!("<{href}>; rel=\"{rel}\""))
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// The published OpenAPI document — the one the contract gate generates from
/// `proto/` and keeps committed (`docs/api/openapi.yaml`), embedded at build
/// time and served as JSON (ADR-0042 D3). Never written by hand here.
pub const OPENAPI_YAML: &str = include_str!("../../../../docs/api/openapi.yaml");

/// `OPENAPI_YAML` as JSON, converted once.
pub fn openapi_document() -> &'static serde_json::Value {
    static DOC: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    DOC.get_or_init(|| {
        serde_yaml::from_str(OPENAPI_YAML)
            .expect("docs/api/openapi.yaml is the generated contract and parses")
    })
}

async fn openapi_json() -> axum::response::Response {
    (hyper::StatusCode::OK, axum::Json(openapi_document())).into_response()
}

/// A path this socket serves, on a method it does not: 405 with `Allow` and a
/// problem document (DX-4001: no handler for this method and path). `allow` is
/// what the contract maps for the path, or `GET` for the documentation routes.
fn method_not_allowed(method: &str, path: &str, allow: &[&str]) -> axum::response::Response {
    let allow = allow.join(", ");
    let mut res = problem_as(
        no_route(format!(
            "route {method} {path} on the node API (this path answers {allow})"
        )),
        path,
        Some(hyper::StatusCode::METHOD_NOT_ALLOWED),
    );
    if let Ok(v) = hyper::header::HeaderValue::from_str(&allow) {
        res.headers_mut().insert(hyper::header::ALLOW, v);
    }
    res
}

/// No handler for this method and path: `NOT_FOUND` carrying DX-4001, which
/// tells it apart from a resource that does not exist (DX-4000).
fn no_route(msg: String) -> Status {
    let mut status = Status::not_found(msg);
    status.metadata_mut().insert(
        crate::network_ops::DX_METADATA,
        tonic::metadata::MetadataValue::from_static("4001"),
    );
    status
}

/// A failure as the engine's error, with its dictionary number, and the HTTP
/// status the REST encoding answers with. `UNIMPLEMENTED` is 501 — the
/// operation is in the contract and this version does not serve it, and a 503
/// would tell a client to retry (its class's status); everything else follows
/// the class.
pub fn engine_error(status: &Status) -> (delonix_model::Error, hyper::StatusCode) {
    use delonix_model::Error;
    let msg = status.message().to_string();
    // A stale `etag` (`If-Match`): 412, never a silent overwrite.
    if status
        .metadata()
        .contains_key(crate::network_ops::STALE_ETAG_METADATA)
    {
        return (Error::Conflict(msg), hyper::StatusCode::PRECONDITION_FAILED);
    }
    // A failure that came from the engine carries its dictionary number: the
    // document says that code, and the status is its class's.
    if let Some(number) = status
        .metadata()
        .get(crate::network_ops::DX_METADATA)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u16>().ok())
    {
        let inner = match number / 1000 {
            1 | 2 => Error::Invalid(msg),
            3 => Error::NotRunning(msg),
            4 => Error::NotFound(msg),
            5 => Error::Conflict(msg),
            6 => Error::Unavailable(msg),
            7 => Error::PermissionDenied(msg),
            8 => Error::Timeout(msg),
            _ => Error::Runtime {
                context: "node-api",
                message: msg,
            },
        };
        let err = Error::coded(number, inner);
        let http = hyper::StatusCode::from_u16(err.class().http_status())
            .unwrap_or(hyper::StatusCode::INTERNAL_SERVER_ERROR);
        return (err, http);
    }
    match status.code() {
        tonic::Code::AlreadyExists => (Error::Conflict(msg), hyper::StatusCode::CONFLICT),
        tonic::Code::InvalidArgument | tonic::Code::FailedPrecondition => {
            (Error::Invalid(msg), hyper::StatusCode::BAD_REQUEST)
        }
        // A resource that is not there. A PATH that is not there carries
        // DX-4001 in its metadata ([`no_route`]) and was answered above.
        tonic::Code::NotFound => (Error::NotFound(msg), hyper::StatusCode::NOT_FOUND),
        tonic::Code::Unimplemented => (
            Error::coded(6001, Error::Unavailable(msg)),
            hyper::StatusCode::NOT_IMPLEMENTED,
        ),
        tonic::Code::PermissionDenied => {
            (Error::PermissionDenied(msg), hyper::StatusCode::FORBIDDEN)
        }
        _ => (
            Error::coded(
                9005,
                Error::Runtime {
                    context: "node-api",
                    message: msg,
                },
            ),
            hyper::StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

/// A failure as the RFC 9457 problem document the OpenAPI declares for every
/// error (ADR-0042 D2): the engine's own `codes::problem` — the same document
/// the CLI's errors map to — with `instance` the request path and
/// `grpc_status` the code the gRPC encoding carries for the same failure.
fn problem(status: Status, instance: &str) -> axum::response::Response {
    problem_as(status, instance, None)
}

/// [`problem`] with an HTTP status other than the failure's own (405 for a
/// method the path does not answer).
fn problem_as(
    status: Status,
    instance: &str,
    http: Option<hyper::StatusCode>,
) -> axum::response::Response {
    let (err, own) = engine_error(&status);
    let http = http.unwrap_or(own);
    let mut doc = delonix_model::codes::problem(&err, Some(instance));
    doc["status"] = http.as_u16().into();
    doc["grpc_status"] = (status.code() as i32).into();
    let mut res = (http, axum::Json(doc)).into_response();
    res.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("application/problem+json"),
    );
    res
}
