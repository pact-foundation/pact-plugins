package main

import "log"
import "net/http"
import "github.com/golang/protobuf/proto"
import "io.pact.plugins.protobuf/provider/io.pact.plugin"

func main() {
    mux := http.NewServeMux()
    mux.HandleFunc("/", func(w http.ResponseWriter, req *http.Request) {
      w.Header().Add("Content-Type", "application/protobuf; message=InitPluginRequest")
      init := &io_pact_plugin.InitPluginRequest{
              Implementation: "Go Provider",
              Version:  "0.0.0",
      }

      out, err := proto.Marshal(init)
      if err != nil {
              log.Fatalln("Failed to encode address init request:", err)
      } else {
        w.Write(out)
      }
    })

    log.Fatal(http.ListenAndServe("localhost:8111", mux))
}
