use crate::network::connectivity::ConnectivityResult;
use kube::api::ListParams;
use kube::{Api, Client};
use k8s_openapi::api::core::v1::{Endpoints, Service};
use std::collections::HashMap;

pub async fn enrich_with_k8s_resources(
    client: &Client,
    namespace: &str,
    results: &mut [ConnectivityResult],
) {
    let namespace_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let all_ns_api: Api<Service> = Api::all(client.clone());

    let mut services: Vec<Service> = Vec::new();
    if let Ok(list) = namespace_api.list(&ListParams::default()).await {
        services.extend(list.items);
    }
    if let Ok(list) = all_ns_api.list(&ListParams::default()).await {
        for svc in list.items {
            if svc.metadata.namespace.as_deref() == Some(namespace) {
                continue;
            }
            services.push(svc);
        }
    }

    let ep_api: Api<Endpoints> = Api::namespaced(client.clone(), namespace);
    let mut ep_map: HashMap<String, Vec<String>> = HashMap::new();
    if let Ok(list) = ep_api.list(&ListParams::default()).await {
        for ep in list.items {
            let ep_name = ep.metadata.name.clone().unwrap_or_default();
            let mut addresses = Vec::new();
            for subset in ep.subsets.unwrap_or_default() {
                for addr in subset.addresses.unwrap_or_default() {
                    addresses.push(addr.ip);
                }
            }
            ep_map.insert(ep_name, addresses);
        }
    }

    let mut ip_map: HashMap<String, (String, u16)> = HashMap::new();
    for svc in &services {
        let svc_name = svc.metadata.name.clone().unwrap_or_default();
        let svc_ns = svc.metadata.namespace.clone().unwrap_or_default();
        let label = if svc_ns == namespace {
            format!("svc/{}", svc_name)
        } else {
            format!("svc/{}.{}", svc_name, svc_ns)
        };

        if let Some(spec) = &svc.spec {
            if let Some(cluster_ip) = &spec.cluster_ip {
                if !cluster_ip.is_empty() && cluster_ip != "None" {
                    for port in &spec.ports {
                        ip_map.insert(
                            format!("{}:{}", cluster_ip, port.port),
                            (label.clone(), port.port as u16),
                        );
                    }
                }
            }
        }

        if let Some(ep_ips) = ep_map.get(&svc_name) {
            for ip in ep_ips {
                if let Some(spec) = &svc.spec {
                    for port in &spec.ports {
                        let key = format!("{}:{}", ip, port.port);
                        if !ip_map.contains_key(&key) {
                            ip_map.insert(
                                key,
                                (format!("ep/{} (svc/{})", ip, svc_name), port.port as u16),
                            );
                        }
                    }
                }
            }
        }
    }

    for r in results.iter_mut() {
        let key = format!("{}:{}", r.target.host, r.target.port);
        if let Some((label, _)) = ip_map.get(&key) {
            r.resource = label.clone();
        } else {
            r.resource = String::new();
        }
    }

    annotate_well_known(results);
}

fn annotate_well_known(results: &mut [ConnectivityResult]) {
    for r in results.iter_mut() {
        if !r.resource.is_empty() {
            continue;
        }
        let port = r.target.port;
        if r.target.host == "10.96.0.1" {
            r.resource = "kubernetes apiserver".into();
        } else if r.target.host == "10.96.0.10" {
            r.resource = "kube-dns/CoreDNS".into();
        } else if port == 6443 {
            r.resource = format!("kube-apiserver ({})", r.target.host);
        } else if port == 10250 {
            r.resource = format!("kubelet ({})", r.target.host);
        } else if port == 10254 {
            r.resource = format!("ingress-nginx ({})", r.target.host);
        } else if port == 2379 {
            r.resource = format!("etcd ({})", r.target.host);
        } else if port == 9100 {
            r.resource = format!("node_exporter ({})", r.target.host);
        } else if port == 53 {
            r.resource = format!("dns ({})", r.target.host);
        }
    }
}
