/**
 * Centralized error store for ImpForge
 *
 * Catches structured errors from Tauri commands (ImpForgeError JSON) and
 * provides a toast-style notification queue with auto-dismiss.
 */

export interface ForgeError {
	category: 'service' | 'validation' | 'file_system' | 'model' | 'browser' | 'config' | 'internal';
	code: string;
	message: string;
	details?: string;
	suggestion?: string;
}

export interface ErrorEntry {
	id: string;
	error: ForgeError;
	timestamp: number;
	dismissed: boolean;
}

const AUTO_DISMISS_MS = 8000;
const MAX_ERRORS = 20;

let errors = $state<ErrorEntry[]>([]);
let nextId = 0;

/**
 * Try to parse a Tauri command error string as a structured ForgeError.
 * Falls back to a generic internal error if parsing fails.
 */
function parseError(errorString: string): ForgeError {
	try {
		const parsed = JSON.parse(errorString);
		if (parsed.category && parsed.code && parsed.message) {
			return parsed as ForgeError;
		}
	} catch {
		// Not JSON — treat as unstructured error string
	}

	return {
		category: 'internal',
		code: 'UNKNOWN',
		message: errorString,
	};
}

function addError(err: ForgeError): string {
	const id = `err-${++nextId}`;
	const entry: ErrorEntry = {
		id,
		error: err,
		timestamp: Date.now(),
		dismissed: false,
	};

	errors = [entry, ...errors].slice(0, MAX_ERRORS);

	// Auto-dismiss after timeout (except internal errors — those persist)
	if (err.category !== 'internal') {
		setTimeout(() => dismiss(id), AUTO_DISMISS_MS);
	}

	return id;
}

function dismiss(id: string) {
	errors = errors.map((e) => (e.id === id ? { ...e, dismissed: true } : e));
	// Clean up fully dismissed after animation time
	setTimeout(() => {
		errors = errors.filter((e) => e.id !== id || !e.dismissed);
	}, 300);
}

function dismissAll() {
	errors = [];
}

function clear() {
	errors = [];
}

/**
 * Wrap a Tauri invoke call with automatic error capturing.
 * Usage: `const result = await errorStore.invoke('command_name', args)`
 */
async function safeInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T | null> {
	try {
		const { invoke } = await import('@tauri-apps/api/core');
		return await invoke<T>(command, args);
	} catch (e) {
		const errorStr = typeof e === 'string' ? e : (e as Error)?.message ?? String(e);
		const nexusErr = parseError(errorStr);
		addError(nexusErr);
		return null;
	}
}

export const errorStore = {
	get errors() {
		return errors.filter((e) => !e.dismissed);
	},
	get all() {
		return errors;
	},
	get hasErrors() {
		return errors.some((e) => !e.dismissed);
	},
	push(errorString: string) {
		return addError(parseError(errorString));
	},
	pushStructured(err: ForgeError) {
		return addError(err);
	},
	dismiss,
	dismissAll,
	clear,
	parseError,
	safeInvoke,
};
