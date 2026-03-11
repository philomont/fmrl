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
     * Returns the age type: 0 = erosion, 1 = fade, 2 = noise
     * @returns {number}
     */
    age_type() {
        const ret = wasm.fmrlview_age_type(this.__wbg_ptr);
        return ret;
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
     * Returns the color mode: 3 = indexed, 6 = RGBA
     * @returns {number}
     */
    color_mode() {
        const ret = wasm.fmrlview_color_mode(this.__wbg_ptr);
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
     * Returns true if this file uses RGBA mode
     * @returns {boolean}
     */
    is_rgba() {
        const ret = wasm.fmrlview_is_rgba(this.__wbg_ptr);
        return ret !== 0;
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
 * Apply one aging step to flat palette indices and return the result.
 *
 * `data` must be `width * height` bytes of palette indices (0–3; 1 = paper).
 * Returns a new array of the same length with aged indices.
 * See `age::age_step` for the full algorithm description.
 * @param {Uint8Array} data
 * @param {number} width
 * @param {number} height
 * @returns {Uint8Array}
 */
export function age_step_indices(data, width, height) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.age_step_indices(ptr0, len0, width, height);
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
 * See `age::consolidation_step` for the full algorithm description.
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
 * Decode a .fmrl file and return flat palette indices (0–3), row-major, width×height bytes.
 * Does not apply decay and does not mutate the file — intended for loading into an editor.
 *
 * Note: For RGBA mode files, this converts RGBA back to indices via quantization.
 * Use `decode_to_rgba` to get raw RGBA data for RGBA mode files.
 * @param {Uint8Array} data
 * @returns {Uint8Array}
 */
export function decode_to_indices(data) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.decode_to_indices(ptr0, len0);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

/**
 * Decode a .fmrl file and return raw RGBA pixels.
 * For indexed mode, this expands palette colors to RGBA.
 * For RGBA mode, this returns the original RGBA data.
 * @param {Uint8Array} data
 * @returns {Uint8Array}
 */
export function decode_to_rgba(data) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.decode_to_rgba(ptr0, len0);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

/**
 * Encode raw RGBA pixels into a new .fmrl file using indexed mode (palette quantization).
 * `rgba` must be `width * height * 4` bytes; dimensions must be multiples of 32.
 * Uses default age_type (erosion).
 * @param {Uint8Array} rgba
 * @param {number} width
 * @param {number} height
 * @returns {Uint8Array}
 */
export function encode_rgba(rgba, width, height) {
    const ptr0 = passArray8ToWasm0(rgba, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.encode_rgba(ptr0, len0, width, height);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

/**
 * Encode raw RGBA pixels into a new .fmrl file using full RGBA mode (no palette quantization).
 * `rgba` must be `width * height * 4` bytes; dimensions must be multiples of 32.
 * Uses default age_type (erosion).
 * @param {Uint8Array} rgba
 * @param {number} width
 * @param {number} height
 * @returns {Uint8Array}
 */
export function encode_rgba_full(rgba, width, height) {
    const ptr0 = passArray8ToWasm0(rgba, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.encode_rgba_full(ptr0, len0, width, height);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

/**
 * Encode raw RGBA pixels in full RGBA mode with specified age type.
 * `age_type`: 0 = erosion, 1 = fade, 2 = noise
 * @param {Uint8Array} rgba
 * @param {number} width
 * @param {number} height
 * @param {number} age_type
 * @returns {Uint8Array}
 */
export function encode_rgba_full_with_age(rgba, width, height, age_type) {
    const ptr0 = passArray8ToWasm0(rgba, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.encode_rgba_full_with_age(ptr0, len0, width, height, age_type);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

/**
 * Encode raw RGBA pixels with specified age type.
 * `age_type`: 0 = erosion, 1 = fade, 2 = noise
 * @param {Uint8Array} rgba
 * @param {number} width
 * @param {number} height
 * @param {number} age_type
 * @returns {Uint8Array}
 */
export function encode_rgba_with_age(rgba, width, height, age_type) {
    const ptr0 = passArray8ToWasm0(rgba, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.encode_rgba_with_age(ptr0, len0, width, height, age_type);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
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
