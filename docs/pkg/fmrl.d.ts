/* tslint:disable */
/* eslint-disable */

export class FmrlView {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Average fade_level across all tiles (0–255).
     */
    avg_fade_level(): number;
    /**
     * Decode and apply decay. Returns RGBA pixels. Also mutates file_bytes.
     */
    decode_and_decay(): Uint8Array;
    /**
     * Return the mutated file bytes for persistence after decode_and_decay.
     */
    get_mutated_bytes(): Uint8Array;
    height(): number;
    /**
     * last_view timestamp (ms since Unix epoch) from tile 0. Returns f64 for JS compatibility.
     */
    last_view_ms(): number;
    static new(data: Uint8Array): FmrlView;
    /**
     * Number of times this image has been viewed (using fade_level of tile 0 as proxy).
     */
    view_count(): number;
    width(): number;
}

/**
 * Create a fresh demo .fmrl file with a manuscript-like pattern.
 * The initial last_view is set 20 days in the past so decay is visible immediately.
 */
export function create_demo_fmrl(): Uint8Array;

/**
 * Decode a .fmrl file and return flat palette indices (0–3), row-major, width×height bytes.
 * Does not apply decay and does not mutate the file — intended for loading into an editor.
 */
export function decode_to_indices(data: Uint8Array): Uint8Array;

/**
 * Encode raw RGBA pixels into a new .fmrl file using the default palette.
 * `rgba` must be `width * height * 4` bytes; dimensions must be multiples of 32.
 */
export function encode_rgba(rgba: Uint8Array, width: number, height: number): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_fmrlview_free: (a: number, b: number) => void;
    readonly create_demo_fmrl: () => [number, number, number, number];
    readonly decode_to_indices: (a: number, b: number) => [number, number, number, number];
    readonly encode_rgba: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly fmrlview_avg_fade_level: (a: number) => number;
    readonly fmrlview_decode_and_decay: (a: number) => [number, number, number, number];
    readonly fmrlview_get_mutated_bytes: (a: number) => [number, number];
    readonly fmrlview_height: (a: number) => number;
    readonly fmrlview_last_view_ms: (a: number) => number;
    readonly fmrlview_new: (a: number, b: number) => [number, number, number];
    readonly fmrlview_view_count: (a: number) => number;
    readonly fmrlview_width: (a: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
