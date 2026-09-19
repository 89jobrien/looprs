#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///

import argparse
import html.parser
import pathlib
import urllib.error
import urllib.parse
import urllib.request


class ReferenceParser(html.parser.HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.references: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        for name, value in attrs:
            if name in {"href", "src"} and value:
                self.references.append(value)


def references_in(path: pathlib.Path) -> list[str]:
    parser = ReferenceParser()
    parser.feed(path.read_text(encoding="utf-8"))
    return parser.references


def local_target(
    source: pathlib.Path, reference: str, site_dir: pathlib.Path
) -> pathlib.Path | None:
    parsed = urllib.parse.urlsplit(reference)
    if parsed.scheme or parsed.netloc or reference.startswith(("mailto:", "tel:")):
        return None
    relative = urllib.parse.unquote(parsed.path)
    if not relative:
        return source
    if relative.startswith("/"):
        target = site_dir / relative.lstrip("/")
    else:
        target = source.parent / relative
    return target.resolve()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Validate and fetch every site href/src"
    )
    parser.add_argument("site_dir", type=pathlib.Path)
    parser.add_argument("base_url")
    args = parser.parse_args()
    site_dir = args.site_dir.resolve()

    failures: list[str] = []
    fetched: set[str] = set()
    references = 0
    sources = sorted(
        path for path in site_dir.iterdir() if path.suffix.lower() in {".html", ".svg"}
    )
    for source in sources:
        for reference in references_in(source):
            references += 1
            target = local_target(source, reference, site_dir)
            if target is None:
                parsed = urllib.parse.urlsplit(reference)
                if parsed.scheme not in {"http", "https", "mailto", "tel"}:
                    failures.append(f"{source.name}: unsupported URL {reference!r}")
                elif parsed.scheme in {"http", "https"} and not parsed.netloc:
                    failures.append(f"{source.name}: invalid URL {reference!r}")
                continue
            try:
                target.relative_to(site_dir)
            except ValueError:
                failures.append(f"{source.name}: reference escapes site: {reference!r}")
                continue
            if not target.is_file():
                failures.append(f"{source.name}: missing local reference {reference!r}")
                continue
            relative = target.relative_to(site_dir).as_posix()
            url = urllib.parse.urljoin(args.base_url.rstrip("/") + "/", relative)
            if url in fetched:
                continue
            try:
                with urllib.request.urlopen(url, timeout=5) as response:
                    if response.status != 200:
                        failures.append(f"{url}: HTTP {response.status}")
            except (urllib.error.URLError, TimeoutError) as error:
                failures.append(f"{url}: {error}")
            fetched.add(url)

    if failures:
        raise SystemExit("\n".join(failures))
    if references == 0:
        raise SystemExit("site contains no href/src references")
    print(
        f"validated {references} href/src references and fetched {len(fetched)} local assets"
    )


if __name__ == "__main__":
    main()
