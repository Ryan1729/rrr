# RRR

RRR is short for *R*yan's *R*SS *R*eader. As the name suggests, this is a project designed for myself, and which displays the content of RSS feeds for the purpose of reading them.

## Design Overview

The particular kinds of RSS feeds I'm interested in are those for websites. As such, any workable design needs to interact with a web browser in some way. Furthermore, many RSS feeds contain things like html entities, etc. that make them only properly displayable in something that would essentially be a web browser anyway.

Given that we need a web browser to display the feeds properly anyway, the best design here seems to be hosting a local web http server and producing pages to be viewed in a web browser. Something that lived entirely inside the browser would be possible, but comes with a lot of restrictions/hassle around things like storage, threading, and so on. And that's why this project involvs a web server.

