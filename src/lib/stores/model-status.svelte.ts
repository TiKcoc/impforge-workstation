/**
 * Model Status Store — tracks which AI models are active, their state, and pipeline flow.
 * Feeds all three visualization levels: Badges, Activity Cards, and Pipeline DAG.
 */

export interface ModelState {
	id: string;
	name: string;
	status: 'idle' | 'thinking' | 'generating' | 'error';
	currentTask: string | null;
	tokensGenerated: number;
	tokensTotal: number | null;
	latencyMs: number;
	routingReason: string | null;
	lastActive: Date;
}

export interface PipelineNode {
	id: string;
	type: 'input' | 'classifier' | 'model' | 'memory' | 'output';
	label: string;
	status: 'idle' | 'active' | 'completed' | 'error';
	x: number;
	y: number;
	metrics?: { tokens: number; latencyMs: number };
}

export interface PipelineEdge {
	from: string;
	to: string;
	active: boolean;
}

class ModelStatusStore {
	models = $state<ModelState[]>([]);
	pipeline = $state<PipelineNode[]>([]);
	edges = $state<PipelineEdge[]>([]);
	lastRouting = $state<{ taskType: string; model: string; reason: string } | null>(null);

	get activeModel() {
		return this.models.find((m) => m.status === 'generating' || m.status === 'thinking') ?? null;
	}

	/** Called when chat_stream sends a Started event */
	onStarted(model: string, taskType: string) {
		// Upsert model state
		const existing = this.models.find((m) => m.name === model);
		if (existing) {
			existing.status = 'generating';
			existing.currentTask = taskType;
			existing.tokensGenerated = 0;
			existing.lastActive = new Date();
		} else {
			this.models.push({
				id: crypto.randomUUID(),
				name: model,
				status: 'generating',
				currentTask: taskType,
				tokensGenerated: 0,
				tokensTotal: null,
				latencyMs: 0,
				routingReason: null,
				lastActive: new Date(),
			});
		}

		// Update pipeline
		this.pipeline = [
			{ id: 'input', type: 'input', label: 'User Input', status: 'completed', x: 0, y: 50 },
			{ id: 'classifier', type: 'classifier', label: `Classifier → ${taskType}`, status: 'completed', x: 150, y: 50 },
			{ id: 'model', type: 'model', label: model.split('/').pop() || model, status: 'active', x: 350, y: 50 },
			{ id: 'memory', type: 'memory', label: 'ForgeMemory', status: 'active', x: 350, y: 120 },
			{ id: 'output', type: 'output', label: 'Output', status: 'idle', x: 550, y: 50 },
		];
		this.edges = [
			{ from: 'input', to: 'classifier', active: false },
			{ from: 'classifier', to: 'model', active: true },
			{ from: 'memory', to: 'model', active: true },
			{ from: 'model', to: 'output', active: false },
		];

		this.lastRouting = { taskType, model, reason: `Task classified as ${taskType}` };
	}

	/** Called on each Delta event */
	onDelta() {
		const active = this.activeModel;
		if (active) {
			active.tokensGenerated += 1;
		}
	}

	/** Called when streaming finishes */
	onFinished(totalTokens: number) {
		const active = this.activeModel;
		if (active) {
			active.status = 'idle';
			active.tokensGenerated = totalTokens;
			active.tokensTotal = totalTokens;
		}

		// Update pipeline
		const modelNode = this.pipeline.find((n) => n.id === 'model');
		const outputNode = this.pipeline.find((n) => n.id === 'output');
		if (modelNode) modelNode.status = 'completed';
		if (outputNode) outputNode.status = 'completed';
		this.edges = this.edges.map((e) => ({ ...e, active: false }));
	}

	/** Called on error */
	onError() {
		const active = this.activeModel;
		if (active) active.status = 'error';
	}

	/** Reset pipeline state */
	reset() {
		this.pipeline = [];
		this.edges = [];
	}
}

export const modelStatus = new ModelStatusStore();
