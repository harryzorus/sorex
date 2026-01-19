/**
 * Sorex Browser Loader
 *
 * Streaming loader for .sorex search indexes with embedded WASM runtime.
 * Supports incremental parallel decoding when SharedArrayBuffer is available.
 */

// =============================================================================
// External Declarations (injected by build.ts from wasm-bindgen output)
// =============================================================================

declare const VERSION: number;
declare const crossOriginIsolated: boolean | undefined;

declare let wasm: WebAssembly.Exports & { memory: WebAssembly.Memory };
declare let cachedUint8ArrayMemory0: Uint8Array | null;
declare let cachedDataViewMemory0: DataView | null;

declare function initSync(opts: { module: BufferSource; memory?: WebAssembly.Memory }): void;
declare function __wbg_get_imports(memory?: WebAssembly.Memory): WebAssembly.Imports;
declare function __wbg_finalize_init(
	instance: WebAssembly.Instance,
	module: WebAssembly.Module
): void;
declare function initThreadPool(threads: number): Promise<void>;

declare class SorexSearcher {
	constructor(bytes: Uint8Array);
	search(
		query: string,
		limit: number,
		onUpdate: (r: SearchResult[]) => void,
		onFinish: (r: SearchResult[]) => void
	): void;
	searchSync(query: string, limit: number): SearchResult[];
	searchWithTierTiming(query: string, limit: number): TierTimingResult;
	doc_count(): number;
	vocab_size(): number;
	free(): void;
}

declare class SorexIncrementalLoader {
	constructor();
	loadHeader(bytes: Uint8Array): RawOffsets;
	loadVocabulary(bytes: Uint8Array): void;
	loadDictTables(bytes: Uint8Array): void;
	loadPostings(bytes: Uint8Array): void;
	loadSuffixArray(bytes: Uint8Array): void;
	loadDocs(bytes: Uint8Array): void;
	loadSectionTable(bytes: Uint8Array): void;
	loadSkipLists(bytes: Uint8Array): void;
	loadLevDfa(bytes: Uint8Array): void;
	finalize(): SorexSearcher;
}

// =============================================================================
// Types
// =============================================================================

interface SearchResult {
	id: number;
	slug: string;
	title: string;
	excerpt: string;
	href: string;
	score: number;
	sectionId: string | null;
	matchType: string;
	matchedTerm: string | null;
}

interface TierTimingResult {
	results: SearchResult[];
	t1Count: number;
	t2Count: number;
	t3Count: number;
	t1TimeUs: number;
	t2TimeUs: number;
	t3TimeUs: number;
}

interface RawOffsets {
	vocabularyStart: number;
	vocabularyEnd: number;
	dictTablesStart: number;
	dictTablesEnd: number;
	postingsStart: number;
	postingsEnd: number;
	suffixArrayStart: number;
	suffixArrayEnd: number;
	docsStart: number;
	docsEnd: number;
	sectionTableStart: number;
	sectionTableEnd: number;
	skipListsStart: number;
	skipListsEnd: number;
	levDfaStart: number;
	levDfaEnd: number;
}

interface SearchCallback {
	onUpdate?: (results: SearchResult[]) => void;
	onFinish?: (results: SearchResult[]) => void;
}

type Range = readonly [start: number, end: number];

// =============================================================================
// Constants
// =============================================================================

const HEADER_SIZE = 52;
const MAGIC = Uint8Array.from([0x53, 0x4f, 0x52, 0x58]); // "SORX"
const FOOTER_MAGIC = Uint8Array.from([0x58, 0x52, 0x4f, 0x53]); // "XROS"

// =============================================================================
// Pure Functions
// =============================================================================

const crc32Table = /* @__PURE__ */ (() =>
	Uint32Array.from({ length: 256 }, (_, i) =>
		Array.from({ length: 8 }).reduce<number>((c) => (c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1), i)
	))();

const crc32 = (data: Uint8Array): number =>
	(data.reduce((crc, byte) => crc32Table[(crc ^ byte) & 0xff] ^ (crc >>> 8), 0xffffffff) ^
		0xffffffff) >>>
	0;

const isSafari = (): boolean =>
	typeof navigator !== 'undefined' &&
	navigator.userAgent.includes('Safari') &&
	!navigator.userAgent.includes('Chrome');

const createSharedMemory = (): WebAssembly.Memory =>
	new WebAssembly.Memory({ initial: 18, maximum: 16384, shared: true });

const getWasmLength = (header: Uint8Array): number =>
	new DataView(header.buffer, header.byteOffset).getUint32(42, true);

const validateMagic = (data: Uint8Array): void => {
	if (!MAGIC.every((b, i) => data[i] === b)) throw new Error('Invalid .sorex file');
	if (data[4] !== VERSION)
		throw new Error(`Version mismatch: file v${data[4]}, loader v${VERSION}`);
};

const sum = (arr: readonly number[]): number => arr.reduce((a, b) => a + b, 0);

const pipe =
	<T>(...fns: Array<(arg: T) => T>) =>
	(x: T): T =>
		fns.reduce((v, f) => f(v), x);

// =============================================================================
// Stream Buffer (functional wrapper over mutable state for performance)
// =============================================================================

interface StreamBuffer {
	readonly length: number;
	push: (chunk: Uint8Array) => void;
	slice: (start: number, end: number) => Uint8Array;
	toArrayBuffer: () => ArrayBuffer;
}

const createStreamBuffer = (): StreamBuffer => {
	const chunks: Uint8Array[] = [];
	let length = 0;

	const slice = (start: number, end: number): Uint8Array => {
		const result = new Uint8Array(end - start);
		let written = 0;
		let pos = 0;

		for (const chunk of chunks) {
			const chunkEnd = pos + chunk.length;
			if (chunkEnd > start && pos < end) {
				const from = Math.max(0, start - pos);
				const to = Math.min(chunk.length, end - pos);
				result.set(chunk.subarray(from, to), written);
				written += to - from;
			}
			pos = chunkEnd;
			if (pos >= end) break;
		}
		return result;
	};

	return {
		get length() {
			return length;
		},
		push: (chunk: Uint8Array) => {
			chunks.push(chunk);
			length += chunk.length;
		},
		slice,
		toArrayBuffer: () => {
			const result = new Uint8Array(length);
			chunks.reduce((offset, chunk) => (result.set(chunk, offset), offset + chunk.length), 0);
			return result.buffer;
		}
	};
};

// =============================================================================
// Binary Format Parser
// =============================================================================

interface ParsedIndex {
	wasmBytes: Uint8Array;
	indexBytes: Uint8Array;
}

const parseIndex = (buffer: ArrayBuffer): ParsedIndex => {
	const data = new Uint8Array(buffer);
	const view = new DataView(buffer);

	validateMagic(data);

	const wasmLen = view.getUint32(42, true);
	const sectionOffsets = [14, 18, 22, 26, 30, 34, 38, 46] as const;
	const sectionLengths = sectionOffsets.map((off) => view.getUint32(off, true));
	const sectionsLen = sum(sectionLengths);
	const sectionsStart = HEADER_SIZE + wasmLen;

	const wasmBytes = data.slice(HEADER_SIZE, sectionsStart);

	// Reconstruct index without WASM
	const contentLen = HEADER_SIZE + sectionsLen;
	const indexBytes = new Uint8Array(contentLen + 8);
	const indexView = new DataView(indexBytes.buffer);

	indexBytes.set(data.subarray(0, HEADER_SIZE));
	indexView.setUint32(42, 0, true); // Zero wasm_len
	indexBytes.set(data.subarray(sectionsStart, sectionsStart + sectionsLen), HEADER_SIZE);
	indexView.setUint32(contentLen, crc32(indexBytes.subarray(0, contentLen)), true);
	FOOTER_MAGIC.forEach((b, i) => (indexBytes[contentLen + 4 + i] = b));

	return { wasmBytes, indexBytes };
};

// =============================================================================
// WASM Runtime
// =============================================================================

let compiledModule: WebAssembly.Module | null = null;

const clearCaches = (): void => {
	cachedUint8ArrayMemory0 = null;
	cachedDataViewMemory0 = null;
};

const initRuntime = async (module: WebAssembly.Module): Promise<void> => {
	const memory = createSharedMemory();
	const instance = await WebAssembly.instantiate(module, __wbg_get_imports(memory));
	__wbg_finalize_init(instance, module);
	clearCaches();
};

const tryInitThreadPool = async (): Promise<boolean> => {
	if (isSafari() || typeof initThreadPool !== 'function') return false;
	try {
		await initThreadPool(navigator?.hardwareConcurrency ?? 4);
		return true;
	} catch {
		return false;
	}
};

const initWasm = async (bytes: Uint8Array): Promise<void> => {
	compiledModule = await WebAssembly.compile(bytes);
	await initRuntime(compiledModule);
	await tryInitThreadPool();
};

const initWasmSync = (bytes: Uint8Array): void => {
	initSync({ module: bytes, memory: createSharedMemory() });
	clearCaches();
};

// =============================================================================
// Section Dispatcher (functional approach with immutable tracking)
// =============================================================================

interface Section {
	readonly range: Range;
	readonly load: (bytes: Uint8Array) => void;
}

const createSections = (loader: SorexIncrementalLoader, offsets: RawOffsets): Section[] => [
	{
		range: [offsets.vocabularyStart, offsets.vocabularyEnd],
		load: (b) => loader.loadVocabulary(b)
	},
	{
		range: [offsets.dictTablesStart, offsets.dictTablesEnd],
		load: (b) => loader.loadDictTables(b)
	},
	{ range: [offsets.postingsStart, offsets.postingsEnd], load: (b) => loader.loadPostings(b) },
	{
		range: [offsets.suffixArrayStart, offsets.suffixArrayEnd],
		load: (b) => loader.loadSuffixArray(b)
	},
	{ range: [offsets.docsStart, offsets.docsEnd], load: (b) => loader.loadDocs(b) },
	{
		range: [offsets.sectionTableStart, offsets.sectionTableEnd],
		load: (b) => loader.loadSectionTable(b)
	},
	{ range: [offsets.skipListsStart, offsets.skipListsEnd], load: (b) => loader.loadSkipLists(b) },
	{ range: [offsets.levDfaStart, offsets.levDfaEnd], load: (b) => loader.loadLevDfa(b) }
];

const dispatchReadySections = (
	sections: Section[],
	dispatched: Set<number>,
	buffer: StreamBuffer
): Set<number> => {
	const newDispatched = new Set(dispatched);
	sections.forEach((section, i) => {
		if (!newDispatched.has(i) && buffer.length >= section.range[1]) {
			section.load(buffer.slice(section.range[0], section.range[1]));
			newDispatched.add(i);
		}
	});
	return newDispatched;
};

// =============================================================================
// Stream Helpers
// =============================================================================

const readChunk = async (
	reader: ReadableStreamDefaultReader<Uint8Array>
): Promise<Uint8Array | null> => {
	const { done, value } = await reader.read();
	return done ? null : value;
};

const readUntil = async (
	reader: ReadableStreamDefaultReader<Uint8Array>,
	buffer: StreamBuffer,
	minBytes: number
): Promise<void> => {
	while (buffer.length < minBytes) {
		const chunk = await readChunk(reader);
		if (!chunk) throw new Error('Unexpected end of stream');
		buffer.push(chunk);
	}
};

const drainStream = async (
	reader: ReadableStreamDefaultReader<Uint8Array>,
	buffer: StreamBuffer
): Promise<void> => {
	let chunk: Uint8Array | null;
	while ((chunk = await readChunk(reader))) {
		buffer.push(chunk);
	}
};

// =============================================================================
// Loaders
// =============================================================================

const loadIncremental = async (url: string): Promise<SorexSearcherWrapper> => {
	const response = await fetch(url);
	if (!response.ok) throw new Error(`Failed to fetch ${url}: ${response.status}`);

	const reader = response.body!.getReader();
	const buffer = createStreamBuffer();

	// Phase 1-2: Read header and WASM
	await readUntil(reader, buffer, HEADER_SIZE);
	const header = buffer.slice(0, HEADER_SIZE);
	const wasmEnd = HEADER_SIZE + getWasmLength(header);

	await readUntil(reader, buffer, wasmEnd);
	compiledModule = await WebAssembly.compile(buffer.slice(HEADER_SIZE, wasmEnd));

	// Phase 3: Initialize runtime
	await initRuntime(compiledModule);

	// Phase 4: Fallback for Safari or thread pool failure
	const fallback = async (): Promise<SorexSearcherWrapper> => {
		await drainStream(reader, buffer);
		return new SorexSearcherWrapper(parseIndex(buffer.toArrayBuffer()).indexBytes);
	};

	if (isSafari() || !(await tryInitThreadPool())) return fallback();

	// Phase 5: Incremental section loading
	const loader = new SorexIncrementalLoader();
	const sections = createSections(loader, loader.loadHeader(header));
	let dispatched = dispatchReadySections(sections, new Set(), buffer);

	while (dispatched.size < sections.length) {
		const chunk = await readChunk(reader);
		if (!chunk) break;
		buffer.push(chunk);
		dispatched = dispatchReadySections(sections, dispatched, buffer);
	}

	return new SorexSearcherWrapper(loader.finalize());
};

const loadSimple = async (url: string): Promise<SorexSearcherWrapper> => {
	const response = await fetch(url);
	if (!response.ok) throw new Error(`Failed to fetch ${url}: ${response.status}`);

	const reader = response.body!.getReader();
	const buffer = createStreamBuffer();

	await drainStream(reader, buffer);

	const { wasmBytes, indexBytes } = parseIndex(buffer.toArrayBuffer());
	await initWasm(wasmBytes);

	return new SorexSearcherWrapper(indexBytes);
};

// =============================================================================
// Public API
// =============================================================================

const OriginalSorexSearcher = SorexSearcher;

class SorexSearcherWrapper {
	private readonly savedWasm: typeof wasm;
	private readonly inner: SorexSearcher;

	constructor(source: Uint8Array | SorexSearcher) {
		this.savedWasm = wasm;
		this.inner =
			source instanceof OriginalSorexSearcher ? source : new OriginalSorexSearcher(source);
	}

	private restore(): void {
		wasm = this.savedWasm;
		clearCaches();
	}

	search(query: string, limit: number, callbacks: SearchCallback = {}): void {
		this.restore();
		this.inner.search(
			query,
			limit,
			callbacks.onUpdate ?? (() => {}),
			callbacks.onFinish ?? (() => {})
		);
	}

	searchSync(query: string, limit: number): SearchResult[] {
		this.restore();
		return this.inner.searchSync(query, limit);
	}

	searchWithTierTiming(query: string, limit: number): TierTimingResult {
		this.restore();
		return this.inner.searchWithTierTiming(query, limit);
	}

	docCount(): number {
		this.restore();
		return this.inner.doc_count();
	}

	vocabSize(): number {
		this.restore();
		return this.inner.vocab_size();
	}

	free(): void {
		this.restore();
		this.inner.free();
	}
}

// =============================================================================
// Entry Points
// =============================================================================

const supportsIncremental = (): boolean =>
	typeof crossOriginIsolated !== 'undefined' &&
	crossOriginIsolated &&
	typeof SorexIncrementalLoader !== 'undefined';

const loadSorex = (url: string): Promise<SorexSearcherWrapper> =>
	supportsIncremental() ? loadIncremental(url) : loadSimple(url);

const loadSorexSync = (buffer: ArrayBuffer): SorexSearcherWrapper => {
	const { wasmBytes, indexBytes } = parseIndex(buffer);
	initWasmSync(wasmBytes);
	return new SorexSearcherWrapper(indexBytes);
};

// =============================================================================
// Exports
// =============================================================================

export { loadSorex, loadSorexSync, SorexSearcherWrapper };
