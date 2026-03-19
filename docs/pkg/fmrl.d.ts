/* tslint:disable */
/* eslint-disable */

export class FmrlView {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Returns the age levels (consolidation levels from fade_level) for all tiles.
     * Each entry is the consolidation level for that tile (0=initial, 1=2x2 done, etc.)
     */
    age_levels(): Uint8Array;
    /**
     * Returns the age types as a comma-separated string (e.g., "0,1,2")
     */
    age_types(): string;
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
     * Returns per-pixel ages extracted from packed tile data.
     * Unpacks low nibble from packed format.
     */
    pixel_ages(): Uint8Array;
    /**
     * Number of times this image has been viewed (using fade_level of tile 0 as proxy).
     */
    view_count(): number;
    width(): number;
}

/**
 * Apply one convolutional bleach step.
 *
 * Uses 2×2 convolution to detect and bleach "noisy" blocks:
 * - If 3+ different indices in 2×2 block → becomes paper
 * - If 2 indices with unequal counts → becomes paper
 * - If 2 indices with equal counts (2 each) AND diagonal pattern → becomes paper
 * See `age::age_by_bleaching` for the full algorithm description.
 */
export function bleach_step_indices(data: Uint8Array, width: number, height: number): Uint8Array;

/**
 * Apply one consolidation step: reduce resolution by 2× then upscale back.
 *
 * `data` must be `width * height` bytes of palette indices.
 * Each 2×2 block becomes one pixel with the most common index (lowest wins ties).
 * Result is upscaled back to original dimensions by duplication.
 * See `age::age_by_consolidation` for the full algorithm description.
 */
export function consolidation_step_indices(data: Uint8Array, width: number, height: number): Uint8Array;

/**
 * Apply one consolidation step with per-pixel ages.
 * Returns [indices_out, pixel_ages_out] as a single concatenated array.
 * indices_out is width*height bytes, pixel_ages_out is width*height bytes.
 */
export function consolidation_step_with_ages(indices: Uint8Array, pixel_ages: Uint8Array, width: number, height: number): Uint8Array;

/**
 * Create a fresh demo .fmrl file with a manuscript-like pattern.
 * The initial last_view is set 20 days in the past so decay is visible immediately.
 */
export function create_demo_fmrl(): Uint8Array;

/**
 * Decode a .fmrl file and return RGB visualization pixels.
 * Format: R = index × 16, G = contrast (0x00 for paper, 0xFF otherwise), B = age × 16
 * Returns 3 bytes per pixel (RGB, no alpha)
 */
export function decode_to_rgb(data: Uint8Array): Uint8Array;

/**
 * Encode raw RGB pixels into a new .fmrl file.
 * `rgb` must be `width * height * 3` bytes; dimensions must be multiples of 128.
 * Format: R = index × 16, G = contrast (0x00 for paper, 0xFF otherwise), B = age × 16
 * `age_types`: array of age type values (0=erosion, 1=consolidation, 2=bleach)
 */
export function encode_rgb(rgb: Uint8Array, width: number, height: number, age_types: Uint8Array): Uint8Array;

/**
 * Encode raw RGB pixels with existing age levels.
 * `age_types`: array of age type values (0=erosion, 1=consolidation, 2=bleach)
 * `age_levels`: per-tile consolidation levels (empty = start fresh)
 */
export function encode_rgb_with_levels(rgb: Uint8Array, width: number, height: number, age_types: Uint8Array, age_levels: Uint8Array): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_fmrlview_free: (a: number, b: number) => void;
    readonly bleach_step_indices: (a: number, b: number, c: number, d: number) => [number, number];
    readonly consolidation_step_indices: (a: number, b: number, c: number, d: number) => [number, number];
    readonly consolidation_step_with_ages: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly create_demo_fmrl: () => [number, number, number, number];
    readonly decode_to_rgb: (a: number, b: number) => [number, number, number, number];
    readonly encode_rgb: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly encode_rgb_with_levels: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
    readonly fmrlview_age_levels: (a: number) => [number, number];
    readonly fmrlview_age_types: (a: number) => [number, number];
    readonly fmrlview_avg_fade_level: (a: number) => number;
    readonly fmrlview_decode_and_decay: (a: number) => [number, number, number, number];
    readonly fmrlview_get_mutated_bytes: (a: number) => [number, number];
    readonly fmrlview_height: (a: number) => number;
    readonly fmrlview_last_view_ms: (a: number) => number;
    readonly fmrlview_new: (a: number, b: number) => [number, number, number];
    readonly fmrlview_pixel_ages: (a: number) => [number, number];
    readonly fmrlview_view_count: (a: number) => number;
    readonly fmrlview_width: (a: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
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
