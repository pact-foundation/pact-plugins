package main

import (
  "log"
  "net/http"
  "github.com/golang/protobuf/proto"
  "google.golang.org/protobuf/types/known/structpb"
  "google.golang.org/protobuf/types/known/wrapperspb"
  "io.pact.plugins.protobuf/provider/io.pact.plugin"
  "encoding/json"
)

type RequestBody struct {
  Description string `json:"description"`
}

func main() {
    mux := http.NewServeMux()
    mux.HandleFunc("/", func(w http.ResponseWriter, req *http.Request) {
      var body RequestBody
      decoder := json.NewDecoder(req.Body)
      err := decoder.Decode(&body)
      if err != nil {
          http.Error(w, err.Error(), http.StatusBadRequest)
          return
      }

      switch body.Description {
        case "Configure Interaction Response":
          log.Println("Configure Interaction Response")
          w.Header().Add("Content-Type", "application/protobuf; message=.io.pact.plugin.InteractionResponse")

          genValue, _ := structpb.NewStruct(map[string]interface{}{
             "format": "YYYY-MM-DD",
          })

          ir := &io_pact_plugin.InteractionResponse{
            Contents: &io_pact_plugin.Body {
              ContentType: "not empty",
              Content:  &wrapperspb.BytesValue{ Value: []byte("[\"jkshdkjadhasjkdh\"]") },
              ContentTypeHint: io_pact_plugin.Body_TEXT,
            },
            Rules: map[string] *io_pact_plugin.MatchingRules {
              "$.abc": &io_pact_plugin.MatchingRules {
                Rule: []*io_pact_plugin.MatchingRule {
                  &io_pact_plugin.MatchingRule {
                    Type: "aasasas",
                  },
                },
              },
            },
            Generators: map[string]*io_pact_plugin.Generator {
              "$.test.one": &io_pact_plugin.Generator {
                Type: "regex",
                Values: genValue,
              },
              "$.test.two": &io_pact_plugin.Generator {
                Type: "equality",
                Values: genValue,
              },
            },
          }

          out, err := proto.Marshal(ir)
          if err != nil {
            http.Error(w, err.Error(), http.StatusBadRequest)
          } else {
            w.Write(out)
          }

        default:
          log.Println("InitPluginRequest default")
          w.Header().Add("Content-Type", "application/protobuf; message=.io.pact.plugin.InitPluginRequest")
          init := &io_pact_plugin.InitPluginRequest{
                 Implementation: "Go Provider",
                 Version:  "0.0.0",
          }

          out, err := proto.Marshal(init)
          if err != nil {
            http.Error(w, err.Error(), http.StatusBadRequest)
          } else {
            w.Write(out)
          }
      }
    })

    log.Fatal(http.ListenAndServe("localhost:8111", mux))
}
