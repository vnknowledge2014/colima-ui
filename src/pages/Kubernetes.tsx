import { useState, useEffect, useCallback } from "react";
import { k8sApi } from "../lib/api";

interface K8sPod {
  name: string; namespace: string; status: string; ready: string;
  restarts: string; age: string; node: string;
}
interface K8sService {
  name: string; namespace: string; svc_type: string; cluster_ip: string;
  ports: string; age: string;
}
interface K8sDeployment {
  name: string; namespace: string; ready: string; available: string; age: string;
}
interface K8sNamespace { name: string; status: string; age: string; }

type Tab = "pods" | "services" | "deployments" | "nodes" | "events";

function parseK8sPods(raw: any): K8sPod[] {
  if (!raw) return [];
  try {
    const parsed = typeof raw === 'string' ? JSON.parse(raw) : raw;
    const items = parsed.items || (Array.isArray(parsed) ? parsed : []);
    return items.map((item: any) => {
      const statuses = item.status?.containerStatuses || [];
      const ready = statuses.filter((s: any) => s.ready).length;
      const restarts = statuses.reduce((s: number, c: any) => s + (c.restartCount || 0), 0);
      return {
        name: item.metadata?.name || "",
        namespace: item.metadata?.namespace || "",
        status: item.status?.phase || "Unknown",
        ready: `${ready}/${statuses.length || item.spec?.containers?.length || 0}`,
        restarts: String(restarts),
        age: item.metadata?.creationTimestamp || "",
        node: item.spec?.nodeName || "",
      };
    });
  } catch { return []; }
}

function parseK8sServices(raw: any): K8sService[] {
  if (!raw) return [];
  try {
    const parsed = typeof raw === 'string' ? JSON.parse(raw) : raw;
    const items = parsed.items || (Array.isArray(parsed) ? parsed : []);
    return items.map((item: any) => ({
      name: item.metadata?.name || "",
      namespace: item.metadata?.namespace || "",
      svc_type: item.spec?.type || "ClusterIP",
      cluster_ip: item.spec?.clusterIP || "None",
      ports: (item.spec?.ports || []).map((p: any) => `${p.port}/${p.protocol}`).join(", "),
      age: item.metadata?.creationTimestamp || "",
    }));
  } catch { return []; }
}

function parseK8sDeployments(raw: any): K8sDeployment[] {
  if (!raw) return [];
  try {
    const parsed = typeof raw === 'string' ? JSON.parse(raw) : raw;
    const items = parsed.items || (Array.isArray(parsed) ? parsed : []);
    return items.map((item: any) => ({
      name: item.metadata?.name || "",
      namespace: item.metadata?.namespace || "",
      ready: `${item.status?.readyReplicas || 0}/${item.spec?.replicas || 0}`,
      available: String(item.status?.availableReplicas || 0),
      age: item.metadata?.creationTimestamp || "",
    }));
  } catch { return []; }
}

function parseK8sNamespaces(raw: any): K8sNamespace[] {
  if (!raw) return [];
  try {
    const parsed = typeof raw === 'string' ? JSON.parse(raw) : raw;
    const items = parsed.items || (Array.isArray(parsed) ? parsed : []);
    return items.map((item: any) => ({
      name: item.metadata?.name || "",
      status: item.status?.phase || "Unknown",
      age: item.metadata?.creationTimestamp || "",
    }));
  } catch { return []; }
}

function timeAgo(ts: string): string {
  if (!ts) return "";
  const diff = Date.now() - new Date(ts).getTime();
  const hours = Math.floor(diff / 3600000);
  if (hours < 1) return `${Math.floor(diff / 60000)}m`;
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}

export default function Kubernetes() {
  const [connected, setConnected] = useState<boolean | null>(null);
  const [tab, setTab] = useState<Tab>("pods");
  const [namespace, setNamespace] = useState("all");
  const [namespaces, setNamespaces] = useState<K8sNamespace[]>([]);
  const [pods, setPods] = useState<K8sPod[]>([]);
  const [services, setServices] = useState<K8sService[]>([]);
  const [deployments, setDeployments] = useState<K8sDeployment[]>([]);
  const [nodesText, setNodesText] = useState("");
  const [eventsText, setEventsText] = useState("");
  const [loading, setLoading] = useState(true);
  const [notification, setNotification] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [selectedPod, setSelectedPod] = useState<K8sPod | null>(null);
  const [podLogs, setPodLogs] = useState("");
  const [describeText, setDescribeText] = useState("");
  const [detailTab, setDetailTab] = useState<"logs" | "describe">("logs");

  const notify = useCallback((type: "success" | "error", text: string) => {
    setNotification({ type, text });
    setTimeout(() => setNotification(null), 4000);
  }, []);

  const checkCluster = useCallback(async () => {
    try {
      await k8sApi.check();
      setConnected(true);
      const nsRaw = await k8sApi.namespaces();
      setNamespaces(parseK8sNamespaces(nsRaw));
    } catch {
      setConnected(false);
    }
    setLoading(false);
  }, []);

  const fetchData = useCallback(async () => {
    if (!connected) return;
    try {
      if (tab === "pods") {
        const raw = await k8sApi.pods(namespace);
        setPods(parseK8sPods(raw));
      } else if (tab === "services") {
        const raw = await k8sApi.services(namespace);
        setServices(parseK8sServices(raw));
      } else if (tab === "deployments") {
        const raw = await k8sApi.deployments(namespace);
        setDeployments(parseK8sDeployments(raw));
      } else if (tab === "nodes") {
        const raw = await k8sApi.nodes();
        setNodesText(raw);
      } else if (tab === "events") {
        const raw = await k8sApi.events(namespace);
        setEventsText(raw);
      }
    } catch (e) {
      notify("error", String(e));
    }
  }, [connected, tab, namespace, notify]);

  useEffect(() => { checkCluster(); }, [checkCluster]);
  useEffect(() => { fetchData(); }, [fetchData]);

  const openPod = async (pod: K8sPod) => {
    setSelectedPod(pod);
    setDetailTab("logs");
    try {
      const [logs, desc] = await Promise.all([
        k8sApi.podLogs(pod.namespace, pod.name, 100),
        k8sApi.describe(pod.namespace, "pod", pod.name),
      ]);
      setPodLogs(logs);
      setDescribeText(desc);
    } catch (e) {
      setPodLogs(`Error: ${e}`);
      setDescribeText(`Error: ${e}`);
    }
  };

  const handleDeletePod = async (pod: K8sPod) => {
    if (!confirm(`Delete pod ${pod.name}?`)) return;
    try {
      await k8sApi.deletePod(pod.namespace, pod.name);
      notify("success", `Pod ${pod.name} deleted`);
      fetchData();
    } catch (e) {
      notify("error", String(e));
    }
  };

  const statusColor = (status: string) => {
    if (status === "Running" || status === "Active" || status === "Available") return "var(--accent-green)";
    if (status === "Pending" || status === "ContainerCreating") return "var(--accent-yellow)";
    if (status === "Failed" || status === "Error" || status === "CrashLoopBackOff") return "var(--accent-red)";
    return "var(--text-secondary)";
  };

  if (loading) {
    return (
      <>
        <div className="content-header"><h1>Kubernetes</h1></div>
        <div className="loading-screen"><div className="spinner" /><span>Connecting to cluster...</span></div>
      </>
    );
  }

  if (!connected) {
    return (
      <>
        <div className="content-header"><h1>Kubernetes</h1></div>
        <div className="content-body">
          <div className="empty-state">
            <div className="empty-state-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="var(--accent-red)" strokeWidth="1.5">
                <circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/>
              </svg>
            </div>
            <div className="empty-state-title">Cluster Not Connected</div>
            <div className="empty-state-text">
              Start Kubernetes in your Colima instance or check kubectl configuration.
            </div>
            <button className="btn btn-primary" onClick={() => { setLoading(true); checkCluster(); }}>Retry Connection</button>
          </div>
        </div>
      </>
    );
  }

  const tabs: { id: Tab; label: string }[] = [
    { id: "pods", label: `Pods (${pods.length})` },
    { id: "services", label: "Services" },
    { id: "deployments", label: "Deployments" },
    { id: "nodes", label: "Nodes" },
    { id: "events", label: "Events" },
  ];

  return (
    <>
      <div className="content-header">
        <h1>
          Kubernetes
          <span style={{ fontSize: "var(--text-sm)", color: "var(--accent-green)", fontWeight: 400, marginLeft: 12 }}>
            ● Connected
          </span>
        </h1>
        <div className="content-header-actions" style={{ display: "flex", gap: 8 }}>
          <select value={namespace} onChange={e => setNamespace(e.target.value)} style={{
            background: "var(--bg-secondary)", border: "1px solid var(--border-primary)",
            borderRadius: 6, padding: "4px 8px", color: "var(--text-primary)",
            fontSize: "var(--text-sm)", fontFamily: "var(--font-mono)",
          }}>
            <option value="all">All Namespaces</option>
            {namespaces.map(ns => (
              <option key={ns.name} value={ns.name}>{ns.name}</option>
            ))}
          </select>
          <button className="btn btn-ghost" onClick={fetchData}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
            </svg>
          </button>
        </div>
      </div>

      <div className="content-body">
        {notification && (
          <div style={{
            position: "fixed", top: 16, right: 16, padding: "10px 16px",
            borderRadius: "var(--radius-md)",
            background: notification.type === "success" ? "rgba(63,185,80,0.15)" : "rgba(248,81,73,0.15)",
            border: `1px solid ${notification.type === "success" ? "var(--accent-green)" : "var(--accent-red)"}`,
            color: notification.type === "success" ? "var(--accent-green)" : "var(--accent-red)",
            fontSize: "var(--text-sm)", zIndex: 200, boxShadow: "var(--shadow-lg)",
          }}>
            {notification.type === "success" ? "✓" : "✕"} {notification.text}
          </div>
        )}

        {/* Tabs */}
        <div style={{ display: "flex", gap: 2, borderBottom: "1px solid var(--border-primary)", marginBottom: 16 }}>
          {tabs.map(t => (
            <button key={t.id} className="btn" onClick={() => setTab(t.id)} style={{
              background: "transparent", border: "none",
              borderBottom: tab === t.id ? "2px solid var(--accent-blue)" : "2px solid transparent",
              color: tab === t.id ? "var(--text-primary)" : "var(--text-secondary)",
              borderRadius: 0, padding: "8px 16px", fontWeight: tab === t.id ? 600 : 400,
            }}>{t.label}</button>
          ))}
        </div>

        {/* Pods */}
        {tab === "pods" && (
          pods.length > 0 ? (
            <div className="card">
              <table className="data-table">
                <thead>
                  <tr>
                    <th>Name</th><th>Namespace</th><th>Status</th><th>Ready</th><th>Restarts</th><th>Age</th><th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {pods.map(pod => (
                    <tr key={`${pod.namespace}/${pod.name}`} onClick={() => openPod(pod)} style={{ cursor: "pointer" }}>
                      <td style={{ fontWeight: 500, fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{pod.name}</td>
                      <td style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>{pod.namespace}</td>
                      <td>
                        <span style={{ color: statusColor(pod.status), fontWeight: 500, fontSize: "var(--text-xs)" }}>
                          ● {pod.status}
                        </span>
                      </td>
                      <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{pod.ready}</td>
                      <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: parseInt(pod.restarts) > 0 ? "var(--accent-yellow)" : "var(--text-muted)" }}>
                        {pod.restarts}
                      </td>
                      <td style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>{timeAgo(pod.age)}</td>
                      <td onClick={e => e.stopPropagation()}>
                        <button className="btn btn-ghost" style={{ fontSize: "var(--text-xs)", color: "var(--accent-red)" }}
                          onClick={() => handleDeletePod(pod)}>Delete</button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="empty-state">
              <div className="empty-state-title">No Pods</div>
              <div className="empty-state-text">No pods found in the selected namespace.</div>
            </div>
          )
        )}

        {/* Services */}
        {tab === "services" && (
          services.length > 0 ? (
            <div className="card">
              <table className="data-table">
                <thead>
                  <tr><th>Name</th><th>Namespace</th><th>Type</th><th>Cluster IP</th><th>Ports</th><th>Age</th></tr>
                </thead>
                <tbody>
                  {services.map(svc => (
                    <tr key={`${svc.namespace}/${svc.name}`}>
                      <td style={{ fontWeight: 500, fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{svc.name}</td>
                      <td style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>{svc.namespace}</td>
                      <td>
                        <span style={{
                          padding: "2px 6px", borderRadius: 4, fontSize: "var(--text-xs)", fontWeight: 500,
                          background: svc.svc_type === "ClusterIP" ? "rgba(88,166,255,0.1)" :
                            svc.svc_type === "NodePort" ? "rgba(63,185,80,0.1)" :
                            svc.svc_type === "LoadBalancer" ? "rgba(188,140,255,0.1)" : "rgba(255,255,255,0.05)",
                          color: svc.svc_type === "ClusterIP" ? "var(--accent-blue)" :
                            svc.svc_type === "NodePort" ? "var(--accent-green)" :
                            svc.svc_type === "LoadBalancer" ? "var(--accent-purple)" : "var(--text-secondary)",
                        }}>{svc.svc_type}</span>
                      </td>
                      <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>{svc.cluster_ip}</td>
                      <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{svc.ports}</td>
                      <td style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>{timeAgo(svc.age)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="empty-state">
              <div className="empty-state-title">No Services</div>
              <div className="empty-state-text">No services found.</div>
            </div>
          )
        )}

        {/* Deployments */}
        {tab === "deployments" && (
          deployments.length > 0 ? (
            <div className="card">
              <table className="data-table">
                <thead>
                  <tr><th>Name</th><th>Namespace</th><th>Ready</th><th>Available</th><th>Age</th></tr>
                </thead>
                <tbody>
                  {deployments.map(dep => (
                    <tr key={`${dep.namespace}/${dep.name}`}>
                      <td style={{ fontWeight: 500, fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{dep.name}</td>
                      <td style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>{dep.namespace}</td>
                      <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)", color: "var(--accent-green)" }}>{dep.ready}</td>
                      <td style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-xs)" }}>{dep.available}</td>
                      <td style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>{timeAgo(dep.age)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="empty-state">
              <div className="empty-state-title">No Deployments</div>
              <div className="empty-state-text">No deployments found.</div>
            </div>
          )
        )}

        {/* Nodes */}
        {tab === "nodes" && (
          <div className="card">
            <pre style={{
              padding: 12, background: "var(--bg-primary)", borderRadius: 8,
              fontSize: "var(--text-xs)", overflow: "auto", maxHeight: "60vh",
              color: "var(--text-secondary)", margin: 0, fontFamily: "var(--font-mono)",
            }}>{nodesText || "No nodes data"}</pre>
          </div>
        )}

        {/* Events */}
        {tab === "events" && (
          <div className="card">
            <pre style={{
              padding: 12, background: "var(--bg-primary)", borderRadius: 8,
              fontSize: "var(--text-xs)", overflow: "auto", maxHeight: "60vh",
              color: "var(--text-secondary)", margin: 0, fontFamily: "var(--font-mono)",
              whiteSpace: "pre-wrap",
            }}>{eventsText || "No events"}</pre>
          </div>
        )}
      </div>

      {/* Pod Detail Modal */}
      {selectedPod && (
        <div className="modal-overlay" onClick={() => setSelectedPod(null)}>
          <div className="modal" onClick={e => e.stopPropagation()} style={{ width: "min(850px, 95vw)", maxHeight: "80vh" }}>
            <div className="modal-header">
              <h2 className="modal-title" style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-md)" }}>
                {selectedPod.name}
                <span style={{ color: statusColor(selectedPod.status), fontSize: "var(--text-sm)", marginLeft: 8 }}>
                  ● {selectedPod.status}
                </span>
              </h2>
              <button className="btn btn-icon btn-ghost" onClick={() => setSelectedPod(null)}>✕</button>
            </div>

            <div style={{ display: "flex", gap: 12, marginBottom: 12, fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
              <span>Namespace: <strong>{selectedPod.namespace}</strong></span>
              <span>Ready: <strong>{selectedPod.ready}</strong></span>
              <span>Restarts: <strong>{selectedPod.restarts}</strong></span>
              <span>Node: <strong>{selectedPod.node}</strong></span>
            </div>

            <div style={{ display: "flex", gap: 2, borderBottom: "1px solid var(--border-primary)", marginBottom: 12 }}>
              <button className="btn" onClick={() => setDetailTab("logs")} style={{
                background: "transparent", border: "none",
                borderBottom: detailTab === "logs" ? "2px solid var(--accent-blue)" : "2px solid transparent",
                color: detailTab === "logs" ? "var(--text-primary)" : "var(--text-secondary)",
                borderRadius: 0, padding: "6px 12px", fontWeight: detailTab === "logs" ? 600 : 400,
              }}>Logs</button>
              <button className="btn" onClick={() => setDetailTab("describe")} style={{
                background: "transparent", border: "none",
                borderBottom: detailTab === "describe" ? "2px solid var(--accent-blue)" : "2px solid transparent",
                color: detailTab === "describe" ? "var(--text-primary)" : "var(--text-secondary)",
                borderRadius: 0, padding: "6px 12px", fontWeight: detailTab === "describe" ? 600 : 400,
              }}>Describe</button>
            </div>

            {detailTab === "logs" && (
              <div className="log-viewer" style={{ maxHeight: "50vh" }}>
                {podLogs.split("\n").map((line, i) => {
                  let cls = "";
                  if (/error|fatal|panic/i.test(line)) cls = "log-error";
                  else if (/warn/i.test(line)) cls = "log-warn";
                  return <div key={i} className={`log-line ${cls}`}>{line}</div>;
                })}
              </div>
            )}

            {detailTab === "describe" && (
              <pre style={{
                padding: 12, background: "var(--bg-primary)", borderRadius: 8,
                fontSize: "var(--text-xs)", overflow: "auto", maxHeight: "50vh",
                color: "var(--text-secondary)", margin: 0, fontFamily: "var(--font-mono)",
                whiteSpace: "pre-wrap",
              }}>{describeText}</pre>
            )}

            <div className="modal-footer">
              <button className="btn btn-ghost" style={{ color: "var(--accent-red)" }}
                onClick={() => { handleDeletePod(selectedPod); setSelectedPod(null); }}>Delete Pod</button>
              <button className="btn btn-primary" onClick={() => setSelectedPod(null)}>Close</button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
