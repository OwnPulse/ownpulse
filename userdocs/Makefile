.PHONY: serve build test docker

serve:  ## Live-reload dev server at http://localhost:8000
	pip install -q -r requirements.txt
	mkdocs serve

build:  ## Build static site to site/
	pip install -q -r requirements.txt
	mkdocs build --strict

test: build  ## Same as CI — strict build catches broken links

docker:  ## Build and run in container at http://localhost:8080
	docker build -t ownpulse-userdocs .
	docker run --rm -p 8080:8080 ownpulse-userdocs
