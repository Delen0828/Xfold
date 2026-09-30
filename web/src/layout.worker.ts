import ELK from 'elkjs/lib/elk-api.js';
import engineUrl from 'elkjs/lib/elk-worker.min.js?url';
const elk = new ELK({ workerFactory: () => new Worker(engineUrl) });
self.onmessage = async ({ data }: MessageEvent) => {
  try {
    const graph = await elk.layout({
      id: 'root', layoutOptions: { 'elk.algorithm': 'layered', 'elk.direction': 'RIGHT', 'elk.spacing.nodeNode': '34', 'elk.layered.spacing.nodeNodeBetweenLayers': '86', 'elk.randomSeed': '1', 'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES' },
      children: data.nodes.map((n: { id: string }) => ({ id: n.id, width: 238, height: 100 })),
      edges: data.edges.map((e: { id: string; from: string; to: string }) => ({ id: e.id, sources: [e.from], targets: [e.to] })),
    });
    self.postMessage({ sequence: data.sequence, hash: data.hash, positions: Object.fromEntries((graph.children || []).map(n => [n.id, { x: n.x || 0, y: n.y || 0 }])) });
  } catch (error) { self.postMessage({ sequence: data.sequence, error: String(error) }); }
  finally { elk.terminateWorker(); }
};
