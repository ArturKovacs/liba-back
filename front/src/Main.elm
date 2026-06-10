port module Main exposing (main)

import Browser
import Browser.Navigation as Nav
import Element exposing (..)
import Element.Background as Background
import Element.Border as Border
import Element.Font as Font
import Element.Input as Input
import Html
import Http
import Json.Encode
import Url
import Url.Parser exposing ((</>), Parser, oneOf, s, top)



-- PORTS


port startWorker : () -> Cmd msg


port subscriptionResultHandler : (String -> msg) -> Sub msg



-- MODEL


type Route
    = Home
    | FloorRoute Int


type alias Model =
    { key : Nav.Key
    , url : Url.Url
    , subscriptionStatus : SubscriptionStatus
    , reportingBananaFoundStatus : ReportingBananaFoundStatus
    }


type alias Flags =
    { isSubscribed : Bool
    }


parseRoute : Url.Url -> Route
parseRoute url =
    case Url.Parser.parse routeParser url of
        Just route ->
            route

        Nothing ->
            Home


routeParser : Parser (Route -> a) a
routeParser =
    oneOf
        [ Url.Parser.map Home top
        , Url.Parser.map FloorRoute (s "floor" </> Url.Parser.int)
        ]


type SubscriptionStatus
    = NotSubscribed
    | Subscribing
    | Subscribed
    | SubscriptionFailed
    | NotificationsDenied
    | SubscriptionStatusUnknown String


type ReportingBananaFoundStatus
    = Idle
    | ReportingBananaFound
    | FinishedReportingBananaFound (Result Http.Error ())


init : Flags -> Url.Url -> Nav.Key -> ( Model, Cmd Msg )
init flags url key =
    ( { key = key
      , url = url
      , subscriptionStatus =
            if flags.isSubscribed then
                Subscribed

            else
                NotSubscribed
      , reportingBananaFoundStatus = Idle
      }
    , Cmd.none
    )



-- UPDATE


type Floor
    = Floor Int


type Msg
    = StartSubscription
    | SubscriptionResultSubscribed
    | SubscriptionResultFailed
    | SubscriptionResultNotificationsDenied
    | SubscriptionResultUnknown String
    | ReportBananaFound Floor -- Send a message to the server which will boradcase it as push messages to everyone
    | ReportBananaFoundResult (Result Http.Error ())
    | LinkClicked Browser.UrlRequest
    | UrlChanged Url.Url


subscriptionResultToMessage : String -> Msg
subscriptionResultToMessage result =
    case result of
        "subscribed" ->
            SubscriptionResultSubscribed

        "failed" ->
            SubscriptionResultFailed

        "notificationsDenied" ->
            SubscriptionResultNotificationsDenied

        other ->
            SubscriptionResultUnknown other


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        StartSubscription ->
            ( { model | subscriptionStatus = Subscribing }, startWorker () )

        SubscriptionResultSubscribed ->
            ( { model | subscriptionStatus = Subscribed }, Cmd.none )

        SubscriptionResultFailed ->
            ( { model | subscriptionStatus = SubscriptionFailed }, Cmd.none )

        SubscriptionResultNotificationsDenied ->
            ( { model | subscriptionStatus = NotificationsDenied }, Cmd.none )

        SubscriptionResultUnknown other ->
            ( { model | subscriptionStatus = SubscriptionStatusUnknown other }, Cmd.none )

        ReportBananaFound (Floor floor) ->
            ( { model | reportingBananaFoundStatus = ReportingBananaFound }
            , Http.post
                { url = "/api/message"

                -- , body = Http.jsonBody (Json.Encode.object [ ( "floor", Json.Encode.int floor ) ])
                , body = Http.stringBody "text/plain" ("Banánt láttak a " ++ (String.fromInt floor) ++ ". emeleten!")
                , expect = Http.expectWhatever ReportBananaFoundResult
                }
            )

        ReportBananaFoundResult result ->
            ( { model | reportingBananaFoundStatus = FinishedReportingBananaFound result }, Cmd.none )

        LinkClicked urlRequest ->
            case urlRequest of
                Browser.Internal url ->
                    ( model, Nav.pushUrl model.key (Url.toString url) )

                Browser.External href ->
                    ( model, Nav.load href )

        UrlChanged url ->
            ( { model | url = url }, Cmd.none )



-- VIEW


main : Program Flags Model Msg
main =
    Browser.application
        { init = init
        , update = update
        , subscriptions = subscriptions
        , view = view
        , onUrlChange = UrlChanged
        , onUrlRequest = LinkClicked
        }


subscriptions : Model -> Sub Msg
subscriptions _ =
    subscriptionResultHandler subscriptionResultToMessage


makeSubscribeButton : Element Msg
makeSubscribeButton =
    Input.button
        [ Border.rounded 10
        , Border.width 2
        , Border.color (rgb255 255 215 0)
        , paddingXY 24 14
        , centerX
        ]
        { onPress = Just StartSubscription
        , label =
            el
                []
                (text "Kérek Push Éretsítéseket")
        }


subscriptionPanel : Model -> Element Msg
subscriptionPanel model =
    case model.subscriptionStatus of
        NotSubscribed ->
            makeSubscribeButton

        Subscribing ->
            el
                [ Font.size 22
                , Font.color (rgb255 255 255 120)
                , centerX
                ]
                (text "Feliratkozás...")

        Subscribed ->
            el
                [ Font.size 22
                , Font.color (rgb255 0 255 180)
                , centerX
                ]
                (text "Feliratkoztál a push értesítésekre.")

        SubscriptionFailed ->
            el
                [ Font.size 22
                , Font.color (rgb255 255 100 100)
                , centerX
                ]
                (text "Nem sikerült feliratkozni a push értesítésekre.")

        NotificationsDenied ->
            el
                [ Font.size 22
                , Font.color (rgb255 255 100 100)
                , centerX
                ]
                (text "Értesítések megtagadva. Engedélyezd a push értesítéseket a böngésződben.")

        SubscriptionStatusUnknown other ->
            el
                [ Font.size 22
                , Font.color (rgb255 255 100 100)
                , centerX
                ]
                (text ("Ismeretlen hiba történt a push értesítések aktiválása során: " ++ other))


view : Model -> Browser.Document Msg
view model =
    let
        currentRoute =
            parseRoute model.url

        content =
            case currentRoute of
                Home ->
                    homeView model

                FloorRoute floorId ->
                    floorView model (Floor floorId)
    in
    { title = "Van Banán?"
    , body = [ content ]
    }


makeFloorLink : Int -> Element Msg
makeFloorLink floorId =
    let
        floorStr =
            String.fromInt floorId
    in
    Element.link
        [ Border.rounded 10
        , Border.width 2
        , Border.color (rgb255 100 200 255)
        , paddingXY 24 14
        , centerX
        ]
        { url = "/floor/" ++ floorStr
        , label =
            el
                []
                (text (floorStr ++ ". Emelet"))
        }


homeView : Model -> Html.Html Msg
homeView model =
    layout
        [ Background.color (rgb255 35 35 35)
        , Font.color (rgb255 255 255 120)
        ]
    <|
        column
            [ width fill
            , height fill
            , spacing 24
            , centerX
            , centerY
            , padding 24
            ]
            ([ el
                [ Font.size 36
                , Font.bold
                , centerX
                ]
                (text "Van Banán?")
             , subscriptionPanel model
             ]
                ++ List.map makeFloorLink [ 0, 1, 2, 3 ]
            )


floorView : Model -> Floor -> Html.Html Msg
floorView model floor =
    let
        floorStr =
            case floor of
                Floor n ->
                    String.fromInt n
    in
    layout
        [ Background.color (rgb255 35 35 35)
        , Font.color (rgb255 255 255 120)
        ]
    <|
        column
            [ width fill
            , height fill
            , spacing 24
            , centerX
            , centerY
            , padding 24
            ]
            [ el
                [ Font.size 36
                , Font.bold
                , centerX
                ]
                (text (floorStr ++ ". Emelet"))
            , subscriptionPanel model
            , Input.button
                [ Border.rounded 10
                , Border.width 2
                , Border.color (rgb255 255 215 0)
                , paddingXY 24 14
                , centerX
                ]
                { onPress = Just (ReportBananaFound floor)
                , label =
                    el
                        []
                        (text "Látok banánt a konyhában!")
                }
            , Element.link
                [ Border.rounded 10
                , Border.width 2
                , Border.color (rgb255 100 200 255)
                , paddingXY 24 14
                , centerX
                ]
                { url = "/"
                , label =
                    el
                        []
                        (text "Emeletek")
                }
            ]
