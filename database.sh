#!/bin/zsh

function docker-db() {
  local dbName=$1
  local port=""
  case $dbName in
    {{#databases}}
    {{shortName}})
      local dbPath="{{dbPath}}"
      local containerName={{containerName}}
      port={{port}}
      ;;
    {{/databases}}
    *)
      echo "no db up function found for: $1"
      return false
      ;;
  esac
  if [[ "$2" == "stop" ]]; then
    docker stop $containerName
  elif [[ "$2" == "clear" ]]; then
    docker stop $containerName
    docker rm $containerName
  elif [[ "$2" == "reset" ]]; then
    docker stop $containerName
    docker rm $containerName
    _freePort $port $containerName
    docker-compose -p $dbName -f $dbPath up -d
  else
    _freePort $port $containerName
    docker-compose -p $dbName -f $dbPath up -d
  fi
}
compdef '_values "docker dbs" {{allDbs}}' docker-db

function _freePort() {
  local toClose=`docker ps | awk -v port="0.0.0.0:$1->" '$0 ~ port { print $NF; exit }'`
  if [ ! -z "$toClose" ] && [ $toClose != $2 ]; then
    echo "stopping $toClose to free up port: $1"
    docker stop $toClose
  fi
}