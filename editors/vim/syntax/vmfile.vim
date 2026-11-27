" Vim/Neovim syntax for Vmfile (also spelled VMfile) — delonix's recipe for a
" bootable qcow2.
"
" It mirrors what `cmd/vmfile.rs` parses, and nothing else:
"
" * one LOGICAL line per instruction — a trailing backslash joins the next
"   physical line, so each instruction is a region that ends on the first line
"   that does not end in `\`;
" * a comment only on a line of its own (`RUN sed -i 's/#a/b/'` keeps its hash);
" * matching is case-INSENSITIVE, because the parser uppercases the first word;
" * any other first word is an error — the parser fails closed on it, and the
"   editor should say so before the build does.
"
" The instruction alternation below is held to `vmfile::INSTRUCTIONS` by a test
" (`editor_grammars_know_exactly_the_instructions`); change both together.

if exists("b:current_syntax")
  finish
endif

" RUN is shell inside the guest.
syn include @vmfileShell syntax/sh.vim
unlet! b:current_syntax

syn case ignore
syn sync fromstart

" Lowest priority first: vim prefers the item defined LAST when two match at
" the same position, so the catch-all comes before the real instructions.
syn match   vmfileUnknown      "^\s*\a[[:alnum:]_-]*"

syn match   vmfileComment      "^\s*#.*$" contains=vmfileTodo
syn keyword vmfileTodo         TODO FIXME XXX NOTE contained

" FROM <image> [AS <stage>]
syn region  vmfileFromLine     matchgroup=vmfileFrom start="^\s*\<from\>" end="\\\@<!$" keepend contains=vmfileImage,vmfileAs,vmfileFlag,vmfileContinuation
syn match   vmfileImage        "\S\+" contained
syn match   vmfileAs           "\<as\>" contained nextgroup=vmfileStage skipwhite
syn match   vmfileStage        "\S\+" contained

" RUN <shell>
syn region  vmfileRunLine      matchgroup=vmfileInstruction start="^\s*\<run\>" end="\\\@<!$" keepend contains=@vmfileShell,vmfileContinuation

" Every other instruction.
syn region  vmfileLine         matchgroup=vmfileInstruction start="^\s*\<\%(copy\|env\|user\|password\|rootpassword\|cloudinit\|sshkey\|size\|hostname\|vcpus\|memory\|hypervisor\|label\|slim\|install\|remove\|autostart\|budget\|kernel\|initrd\|boot\)\>" end="\\\@<!$" keepend contains=vmfileFlag,vmfileString,vmfileNumber,vmfileKey,vmfileContinuation

" --name=value and bare --name (AUTOSTART --only).
syn match   vmfileFlag         "--\a[[:alnum:]-]*" contained nextgroup=vmfileFlagEq
syn match   vmfileFlagEq       "=" contained nextgroup=vmfileOnOff,vmfileFlagValue
syn match   vmfileFlagValue    "[^[:space:],]\+" contained nextgroup=vmfileComma
syn match   vmfileComma        "," contained nextgroup=vmfileFlagValue
syn match   vmfileOnOff        "\<\%(on\|off\)\>\ze\%(\s\|$\)" contained

" ENV KEY=value, LABEL key=value.
syn match   vmfileKey          "\%(^\|\s\)\zs[[:alpha:]_][[:alnum:]_.-]*\ze=" contained

" SIZE 4G, BUDGET 450M, MEMORY 1G, VCPUS 2.
syn match   vmfileNumber       "\%(^\|\s\)\zs\d\+[GMK]\=\ze\%(\s\|$\)" contained

syn region  vmfileString       start=+"+ skip=+\\"+ end=+"+ oneline contained
syn region  vmfileString       start=+'+ end=+'+ oneline contained

" A trailing backslash joins the next line; seeing it is how you notice it is
" missing.
syn match   vmfileContinuation "\\$" contained

syn case match

hi def link vmfileUnknown      Error
hi def link vmfileComment      Comment
hi def link vmfileTodo         Todo
hi def link vmfileFrom         Keyword
hi def link vmfileInstruction  Keyword
hi def link vmfileAs           Keyword
hi def link vmfileImage        String
hi def link vmfileStage        Type
hi def link vmfileFlag         Special
hi def link vmfileFlagEq       Operator
hi def link vmfileFlagValue    String
hi def link vmfileComma        Delimiter
hi def link vmfileOnOff        Boolean
hi def link vmfileKey          Identifier
hi def link vmfileNumber       Number
hi def link vmfileString       String
hi def link vmfileContinuation Special

let b:current_syntax = "vmfile"
