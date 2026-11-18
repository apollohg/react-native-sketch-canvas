# The exact set of UniFFI functions every native artefact exports, after `uniffi_sketch_core_fn_`.
SKETCH_SYMBOL_PREFIX="uniffi_sketch_core_fn_"
SKETCH_SYMBOLS=(
    clone_sketchsession
    free_sketchsession
    constructor_sketchsession_from_json
    constructor_sketchsession_new_empty
    func_fit_transform
    func_parse_background
    method_sketchsession_begin_stroke
    method_sketchsession_cancel_stroke
    method_sketchsession_clear
    method_sketchsession_committed_paths
    method_sketchsession_document_size
    method_sketchsession_end_stroke
    method_sketchsession_export_svg
    method_sketchsession_extend_stroke
    method_sketchsession_has_live_stroke
    method_sketchsession_load_json
    method_sketchsession_redo
    method_sketchsession_state
    method_sketchsession_to_json
    method_sketchsession_undo
)

# Usage: verify_sketch_symbols LABEL < symbol-listing
# Fails unless the listing exports exactly SKETCH_SYMBOLS under the prefix.
verify_sketch_symbols() {
    local label="$1"
    local actual expected
    actual="$({ grep -o "${SKETCH_SYMBOL_PREFIX}[a-z_]*" || true; } | sort -u)"
    expected="$(printf "${SKETCH_SYMBOL_PREFIX}%s\n" "${SKETCH_SYMBOLS[@]}" | sort -u)"
    if [[ "$actual" != "$expected" ]]; then
        echo "error: $label exports a different UniFFI surface than rust/symbols.sh:" >&2
        diff <(echo "$expected") <(echo "$actual") >&2 || true
        return 1
    fi
    echo "  $label: ${#SKETCH_SYMBOLS[@]} symbols"
}
