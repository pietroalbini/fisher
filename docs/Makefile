.PHONY: build update-snowflake

build:
	@mkdocs build --clean

update-snowflake:
	@snowflake-css -m --color "#00695c" typography navbar sidebar inline-list footer > theme/css/snowflake.css
