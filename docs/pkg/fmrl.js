/* @ts-self-types="./fmrl.d.ts" */

export class FmrlView {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(FmrlView.prototype);
        obj.__wbg_ptr = ptr;
        FmrlViewFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        FmrlViewFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_fmrlview_free(ptr, 0);
    }
    /**
     * Returns the age levels (consolidation levels from fade_level) for all tiles.
     * Each entry is the consolidation level for that tile (0=initial, 1=2x2 done, etc.)
     * @returns {Uint8Array}
     */
    age_levels() {
        const ret = wasm.fmrlview_age_levels(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Returns the age types as a comma-separated string (e.g., "0,1,2")
     * @returns {string}
     */
    age_types() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.fmrlview_age_types(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Average fade_level across all tiles (0–255).
     * @returns {number}
     */
    avg_fade_level() {
        const ret = wasm.fmrlview_avg_fade_level(this.__wbg_ptr);
        return ret;
    }
    /**
     * Decode and apply decay. Returns RGBA pixels. Also mutates file_bytes.
     * @returns {Uint8Array}
     */
    decode_and_decay() {
        const ret = wasm.fmrlview_decode_and_decay(this.__wbg_ptr);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Return the mutated file bytes for persistence after decode_and_decay.
     * @returns {Uint8Array}
     */
    get_mutated_bytes() {
        const ret = wasm.fmrlview_get_mutated_bytes(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * @returns {number}
     */
    height() {
        const ret = wasm.fmrlview_height(this.__wbg_ptr);
        return ret;
    }
    /**
     * last_view timestamp (ms since Unix epoch) from tile 0. Returns f64 for JS compatibility.
     * @returns {number}
     */
    last_view_ms() {
        const ret = wasm.fmrlview_last_view_ms(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {Uint8Array} data
     * @returns {FmrlView}
     */
    static new(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.fmrlview_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return FmrlView.__wrap(ret[0]);
    }
    /**
     * Returns per-pixel ages extracted from packed tile data.
     * Unpacks low nibble from packed format.
     * @returns {Uint8Array}
     */
    pixel_ages() {
        const ret = wasm.fmrlview_pixel_ages(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Number of times this image has been viewed (using fade_level of tile 0 as proxy).
     * @returns {number}
     */
    view_count() {
        const ret = wasm.fmrlview_view_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    width() {
        const ret = wasm.fmrlview_width(this.__wbg_ptr);
        return ret;
    }
}
if (Symbol.dispose) FmrlView.prototype[Symbol.dispose] = FmrlView.prototype.free;

/**
 * Apply one convolutional bleach step.
 *
 * Uses 2×2 convolution to detect and bleach "noisy" blocks:
 * - If 3+ different indices in 2×2 block → becomes paper
 * - If 2 indices with unequal counts → becomes paper
 * - If 2 indices with equal counts (2 each) AND diagonal pattern → becomes paper
 * See `age::age_by_bleaching` for the full algorithm description.
 * @param {Uint8Array} data
 * @param {number} width
 * @param {number} height
 * @returns {Uint8Array}
 */
export function bleach_step_indices(data, width, height) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.bleach_step_indices(ptr0, len0, width, height);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

/**
 * Apply one consolidation step: reduce resolution by 2× then upscale back.
 *
 * `data` must be `width * height` bytes of palette indices.
 * Each 2×2 block becomes one pixel with the most common index (lowest wins ties).
 * Result is upscaled back to original dimensions by duplication.
 * See `age::age_by_consolidation` for the full algorithm description.
 * @param {Uint8Array} data
 * @param {number} width
 * @param {number} height
 * @returns {Uint8Array}
 */
export function consolidation_step_indices(data, width, height) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.consolidation_step_indices(ptr0, len0, width, height);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

/**
 * Apply one consolidation step with per-pixel ages.
 * Returns [indices_out, pixel_ages_out] as a single concatenated array.
 * indices_out is width*height bytes, pixel_ages_out is width*height bytes.
 * @param {Uint8Array} indices
 * @param {Uint8Array} pixel_ages
 * @param {number} width
 * @param {number} height
 * @returns {Uint8Array}
 */
export function consolidation_step_with_ages(indices, pixel_ages, width, height) {
    const ptr0 = passArray8ToWasm0(indices, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(pixel_ages, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.consolidation_step_with_ages(ptr0, len0, ptr1, len1, width, height);
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}

/**
 * Create a fresh demo .fmrl file with a manuscript-like pattern.
 * The initial last_view is set 20 days in the past so decay is visible immediately.
 * @returns {Uint8Array}
 */
export function create_demo_fmrl() {
    const ret = wasm.create_demo_fmrl();
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v1;
}

/**
 * Decode a .fmrl file and return RGB visualization pixels.
 * Format: R = index × 16, G = contrast (0x00 for paper, 0xFF otherwise), B = age × 16
 * Returns 3 bytes per pixel (RGB, no alpha)
 * @param {Uint8Array} data
 * @returns {Uint8Array}
 */
export function decode_to_rgb(data) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.decode_to_rgb(ptr0, len0);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

/**
 * Encode raw RGB pixels into a new .fmrl file.
 * `rgb` must be `width * height * 3` bytes; dimensions must be multiples of 128.
 * Format: R = index × 16, G = contrast (0x00 for paper, 0xFF otherwise), B = age × 16
 * `age_types`: array of age type values (0=erosion, 1=consolidation, 2=bleach)
 * @param {Uint8Array} rgb
 * @param {number} width
 * @param {number} height
 * @param {Uint8Array} age_types
 * @returns {Uint8Array}
 */
export function encode_rgb(rgb, width, height, age_types) {
    const ptr0 = passArray8ToWasm0(rgb, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(age_types, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ret = wasm.encode_rgb(ptr0, len0, width, height, ptr1, len1);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v3;
}

/**
 * Encode raw RGB pixels with existing age levels.
 * `age_types`: array of age type values (0=erosion, 1=consolidation, 2=bleach)
 * `age_levels`: per-tile consolidation levels (empty = start fresh)
 * @param {Uint8Array} rgb
 * @param {number} width
 * @param {number} height
 * @param {Uint8Array} age_types
 * @param {Uint8Array} age_levels
 * @returns {Uint8Array}
 */
export function encode_rgb_with_levels(rgb, width, height, age_types, age_levels) {
    const ptr0 = passArray8ToWasm0(rgb, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ptr1 = passArray8ToWasm0(age_types, wasm.__wbindgen_malloc);
    const len1 = WASM_VECTOR_LEN;
    const ptr2 = passArray8ToWasm0(age_levels, wasm.__wbindgen_malloc);
    const len2 = WASM_VECTOR_LEN;
    const ret = wasm.encode_rgb_with_levels(ptr0, len0, width, height, ptr1, len1, ptr2, len2);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v4 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v4;
}

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_6ddd609b62940d55: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_now_16f0c993d5dd6c27: function() {
            const ret = Date.now();
            return ret;
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./fmrl_bg.js": import0,
    };
}

const FmrlViewFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_fmrlview_free(ptr >>> 0, 1));

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('fmrl_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
