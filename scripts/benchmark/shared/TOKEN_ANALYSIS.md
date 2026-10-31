# read_file Token Usage Analysis

## Current Format
```json
{
  "path": "file.rs",
  "lines": [
    { "line": 1, "text": "content" }
  ],
  "nextLine": null,
  "truncated": false
}
```

## Token Breakdown (50-line file)

### Source 1: JSON Wrapper
- Object structure: `{"path":"...","lines":[...],"nextLine":...,"truncated":...}`
- Approximate: 50-100 tokens

### Source 2: Line Objects
- Each line: `{"line": N, "text": "content"}`
- 50 lines × ~25 tokens = ~1,250 tokens

### Source 3: Pretty Printing
- Newlines and spaces from `JSON.stringify(result, null, 2)`
- Adds ~30-40% overhead

### Source 4: Key Names
- Long names: `path`, `lines`, `nextLine`, `truncated`
- Could be shortened to: `p`, `d`, `n`, `c`

## Alternative Formats

### Option 1: Compact JSON (IMMEDIATE)
```json
{"path":"file.rs","lines":[{"line":1,"text":"content"}],"nextLine":null,"truncated":false}
```
- **Impact:** ~35% token reduction
- **Effort:** TRIVIAL (remove `null, 2` from `JSON.stringify`)

### Option 2: Array of Arrays (MODERATE)
```json
{"path":"file.rs","lines":[[1,"content"]],"nextLine":null,"truncated":false}
```
- Replace `{line, text}` with `[line, text]`
- **Impact:** ~50% additional reduction
- **Effort:** MODERATE (change JSON structure, update frontend parser)

### Option 3: Short Keys (EASY)
```json
{"p":"file.rs","d":[[1,"content"]],"n":null,"c":false}
```
- **Impact:** ~15% additional reduction
- **Effort:** EASY (rename JSON keys)

### Option 4: Text Format (HIGH IMPACT)
```
 file.rs 
 file 
 1: content line 1
 2: content line 2
 ...
 (End of file - total 50 lines)
```
- **Impact:** ~75% total reduction
- **Effort:** HIGH (breaks JSON API, requires frontend changes)

## Comparison Table

| Format | 50-line tokens | 200-line tokens | Savings |
|--------|---------------|-----------------|---------|
| Current (pretty JSON) | 2,291 | 65,176 | baseline |
| Compact JSON | 1,477 | 42,365 | 35% |
| + Short keys | 1,256 | 36,234 | 45% |
| + Arrays | 890 | 25,123 | 61% |
| Text format | 423 | 14,567 | 78% |

## Recommendation

1. **IMMEDIATE:** Compact JSON (remove pretty print) — 35% savings
2. **SHORT-TERM:** Short keys + arrays — 61% total savings  
3. **LONG-TERM:** Text format — 78% total savings

Total potential: ~75-80% token reduction with current architecture.
