#!/usr/bin/env node

if (process.argv.length !== 3) {
    console.error(`
Usage: ${process.argv[1]} ./tests/commonmark.rs
`)
    process.exit(1)
}

import fs from 'fs/promises'
import testgen from 'markdown-it-testgen'


function rust_escape(s) {
    if (s.match(/( $|\t)/m)) {
        return '"' + s.replace(/\\/g, "\\\\").replace(/"/g, "\\\"").replace(/ $/mg, '\\x20').replace(/\t/g, '\\t') + '"'
    }

    if (s.match(/#"|"#/)) return 'r##"' + s + '"##'
    return 'r#"' + s + '"#'
}

let identmap = new Set()
function ident(str) {
    str = str.toLowerCase()
            .replace(/[^a-z0-9]+/g, '_')
            .replace(/^_+|_+$/g, '')

    if (!str) str = 'unnamed'

    let result = str
    let idx = 0
    while (identmap.has(result)) {
        result = str + '_' + (++idx)
    }

    identmap.add(result)
    return result
}

function generate(fixture) {
    return `
#[test]
fn ${ident(fixture.header)}() {
    let input = ${rust_escape(fixture.first.text.replace(/\n$/, ''))};
    let output = ${rust_escape(fixture.second.text.replace(/\n$/, ''))};
    run(input, output);
}
`.trim()
}

let input = await fs.readFile(process.argv[2], 'utf8')
let lines = []
let state = 'passthrough'

for (let line of input.split('\n')) {
    switch(state) {
        case 'passthrough':
            lines.push(line)
            if (line.match(/^\/{4,}$/)) {
                state = 'maybeheader'
            }
            break
        case 'skipping':
            if (line.match(/^\/{4,}$/)) {
                lines.push(line)
                state = 'maybeheader'
            }
            break
        case 'maybeheader':
            lines.push(line)
            let match = line.match(/^\/{2,}\s+TESTGEN:\s*(.+)\s*$/)
            if (match) {
                let has_data = false
                lines.push('#[rustfmt::skip]')
                lines.push(`mod ${ident(match[1])} {`)
                lines.push('use super::run;')
                lines.push('// this part of the file is auto-generated')
                lines.push('// don\'t edit it, otherwise your changes might be lost')
                testgen.load(match[1], data => {
                    data.fixtures.forEach((data, idx) => {
                        has_data = true
                        if (idx++ > 0) lines.push('')
                        let generated = generate(data).replace(/\r?\n$/, '')
                        lines = lines.concat(generated.split('\n'))
                    })
                })
                lines.push('// end of auto-generated module')
                lines.push('}')
                if (!has_data) throw new Error(`no data found for ${match[1]}`)
                state = 'skipping'
            }
            break
        default:
            throw Error('unknown state')
    }
}

await fs.rename(process.argv[2], process.argv[2] + '.old')
await fs.writeFile(process.argv[2], lines.join('\n') + '\n')
