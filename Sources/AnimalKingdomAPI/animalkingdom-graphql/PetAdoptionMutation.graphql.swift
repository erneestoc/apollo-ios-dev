// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

class PetAdoptionMutation: GraphQLMutation {
  static let operationName: String = "PetAdoptionMutation"
  static let operationDocument: Apollo.OperationDocument = .init(
    operationIdentifier: "a8bb3d5a88077b77696636b1ba6f27e9aa5952660d53b84b44c7f3403f231ac1"
  )

  var input: TestSchema.PetAdoptionInput

  init(input: TestSchema.PetAdoptionInput) {
    self.input = input
  }

  var __variables: Variables? { ["input": input] }

  struct Data: TestSchema.SelectionSet {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Mutation }
    static var __selections: [Apollo.Selection] { [
      .field("adoptPet", AdoptPet.self, arguments: ["input": .variable("input")]),
    ] }

    var adoptPet: AdoptPet { __data["adoptPet"] }

    /// AdoptPet
    ///
    /// Parent Type: `Pet`
    struct AdoptPet: TestSchema.SelectionSet {
      let __data: DataDict
      init(_dataDict: DataDict) { __data = _dataDict }

      static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.Pet }
      static var __selections: [Apollo.Selection] { [
        .field("__typename", String.self),
        .field("id", TestSchema.ID.self),
        .field("humanName", String?.self),
      ] }

      var id: TestSchema.ID { __data["id"] }
      var humanName: String? { __data["humanName"] }
    }
  }
}
