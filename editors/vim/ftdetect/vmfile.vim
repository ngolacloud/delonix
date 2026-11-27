" A Vmfile is recognised by its NAME, not by an extension — it has none, the
" same way a Dockerfile has none. `Vmfile` is what `vm image init` writes;
" `VMfile` is the older spelling of the same format. `Vmfile.dev`/`dev.Vmfile`
" are the two shapes people reach for when a project holds more than one.
au BufNewFile,BufRead Vmfile,VMfile,Vmfile.*,VMfile.*,*.Vmfile,*.VMfile,*.vmfile setfiletype vmfile
